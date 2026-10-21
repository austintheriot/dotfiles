import {spawn} from 'node:child_process'
import {copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync} from 'node:fs'
import {basename, join} from 'node:path'
import {z} from 'zod'
import type {Command, MarkKind} from './args'
import {BASE_IMAGE, CHROME_PORT, EXIT, FONTS_DIR, PULL_DIR, VIDEO_DIR, VM_BUN, VM_SCREEN, VM_TOOL_FILES, VM_VIDEO_TOOLS, vmName} from './config'
import {bootVm} from './boot'
import {startChrome} from './chrome'
import {strayChromePids} from './guest'
import {approveStep} from './image/approve'
import {baseImageState, readHostImageState, readMarkers, withBaseImage} from './image/base-image'
import {copyGuestTools, importCookies} from './image/guest-tools'
import {chromePolicyDigest} from './image/grants'
import {agentPath, grantStep, LOCAL_NETWORK_SCRIPTED, rewriteReplaydApproval} from './image/grant'
import {approvalWarning, planImageBuild} from './image/plan'
import {BUN_VERSION, provisionStep, readProvisionFiles} from './image/provision'
import {loadSession} from './image/session-store'
import {reportGoogleSession, signinStep} from './image/signin'
import {planWarm, warmStep, type WarmPlan} from './image/warm'
import {confirmStopped, describeVms, planRestart} from './lease'
import {diagnoseNetwork, parseProbe, PROBE_SCRIPT} from './network'
import {stopFailure} from './recorder'
import {prepareLease, processTable, readLeases, readOwnership, releaseLease, takeLease, tartVms} from './leases'
import type {Profile} from './profile'
import {hostStdinCarriesData, run, runExecWithRetry, shouldDiagnoseNetwork, succeeds, tartExec, VmError, warn, zsh} from './proc'
import {copyCheckout, fontCopies, inputShimScript} from './worktree'

// The warm plan reads the secrets and hashes the lockfiles, so it runs before the boot and fails early.
// readProvisionFiles runs inside the provision step instead: markers live in the guest, so only the booted
// image says whether provision is due, and a build that does not need provision never needs the files.
async function imageBuild(checkout: string | undefined, profile: Profile): Promise<void> {
  if (checkout !== undefined && profile.warm === undefined) console.log('skipping warm: the vm profile defines no warm step')
  const warm: WarmPlan | undefined = checkout === undefined || profile.warm === undefined ? undefined : planWarm(checkout, profile.warm)
  const origins = profile.chrome.mediaOrigins
  await withBaseImage({window: true}, async vm => {
    const chrome = tartExec(vm, 'defaults read "/Applications/Google Chrome.app/Contents/Info" CFBundleShortVersionString').stdout.trim()
    const current = {
      provision: {bun: BUN_VERSION},
      grant: {agent: agentPath(vm), localNetwork: String(LOCAL_NETWORK_SCRIPTED), policy: chromePolicyDigest(origins)},
      approve: {chrome},
      ...(warm === undefined ? {} : {warm: warm.inputs})
    }
    const plan = planImageBuild(readMarkers(vm), current, {checkout: warm !== undefined, now: new Date()})
    plan.skipped.forEach(skip => console.log(`skipping ${skip.step}: ${skip.reason}`))
    for (const step of plan.run) {
      console.log(`==> vm image ${step}`)
      if (step === 'provision') provisionStep(vm, readProvisionFiles(profile.provision.files))
      else if (step === 'grant') grantStep(vm, origins)
      else if (step === 'approve') await approveStep(vm, origins)
      else if (step === 'warm' && warm !== undefined) warmStep(vm, warm, profile.worktree.paths)
      else if (step === 'signin') await signinStep(vm, profile)
    }
  })
}

type Env = Record<string, string | undefined>

// Tart's virtual sound input passes through to the host microphone, and opening it hangs the guest's
// Core Audio, so getUserMedia never resolves. The VM runs with --no-audio and BlackHole 2ch as its only input.
function ensureLoopbackMic(vm: string): void {
  if (tartExec(vm, 'system_profiler SPAudioDataType 2>/dev/null | grep -q BlackHole').code === 0) return
  console.error(`installing BlackHole 2ch in ${vm}; rebuild ${BASE_IMAGE} to bake it into the image`)
  tartExec(vm, 'brew install --quiet --cask blackhole-2ch && sudo killall coreaudiod')
  if (tartExec(vm, 'sleep 3; system_profiler SPAudioDataType 2>/dev/null | grep -q BlackHole').code !== 0)
    warn(`${vm} has no BlackHole input, so desktop recording steps will hang at getUserMedia`)
}

// tart set --display changes the virtual display, but macOS keeps the mode it chose on first boot.
// The desktop app opens at 1440x900, so a 1024x768 screen cuts off its right edge.
function ensureDisplayMode(vm: string): void {
  tartExec(
    vm,
    `command -v displayplacer >/dev/null || brew install --quiet displayplacer >/dev/null 2>&1
    id=$(displayplacer list | sed -n "s/^Persistent screen id: //p" | head -1)
    displayplacer "id:$id res:${VM_SCREEN.width}x${VM_SCREEN.height} hz:60 color_depth:7 enabled:true scaling:on origin:(0,0) degree:0"`
  )
  if (tartExec(vm, `displayplacer list | grep -q "res:${VM_SCREEN.width}x${VM_SCREEN.height} .*scaling:on <-- current mode"`).code !== 0)
    warn(`${vm} is not at ${VM_SCREEN.width}x${VM_SCREEN.height}, so the desktop app window will be cut off`)
}

// QA input goes through real macOS mouse and key events, sent with cliclick, so the cursor shows in videos.
function ensureCliclick(vm: string): void {
  if (tartExec(vm, 'command -v cliclick >/dev/null || brew install --quiet cliclick >/dev/null 2>&1; command -v cliclick >/dev/null').code !== 0)
    warn(`${vm} has no cliclick, so QA input falls back to CDP and videos show no cursor`)
}

// The recorder and the input-logging proxy run in the VM, so every timestamp uses the VM clock.
// The editor runs on the host, which has ffmpeg, Chrome, and the fonts the profile names.
function copyVideoTools(vm: string, worktree: string, fonts: Profile['fonts']): void {
  zsh(`tar -C "$1" -cf - ${VM_TOOL_FILES.join(' ')} | tart exec -i "$2" zsh -c "rm -rf $3 && mkdir -p $3 && tar -xf - -C $3"`, [VIDEO_DIR, vm, VM_VIDEO_TOOLS])
  rmSync(FONTS_DIR, {recursive: true, force: true})
  mkdirSync(FONTS_DIR, {recursive: true})
  if (fonts === undefined) return
  try {
    fontCopies(worktree, FONTS_DIR, fonts).forEach(copy => copyFileSync(copy.from, copy.to))
  } catch {
    console.error(`the fonts the vm profile names are not in ${worktree}; QA videos fall back to system fonts`)
  }
}

// A CDP driver script sends its clicks and keys over CDP, so they never become OS events the screen
// recorder can show. The VM copy of each driver the profile names becomes a shim that sends them as real
// macOS input, logs each one to the video timeline, and uses the original, now cdp.real.mjs, to find targets.
function installInputShims(vm: string, drivers: readonly string[]): void {
  for (const driver of drivers) {
    if (tartExec(vm, inputShimScript(driver, VM_VIDEO_TOOLS)).code !== 0)
      warn(`input logging is not installed for ${driver} in ${vm}, so its clicks and keys will not show in QA videos`)
  }
}

// A Chrome without the QA profile cannot be driven over CDP and covers the one that can. Login can reopen it
// late, so the check runs right before startChrome.
function quitStrayChrome(vm: string): void {
  const home = tartExec(vm, 'print -r -- $HOME')
  const listing = tartExec(vm, 'ps -axo pid=,command=')
  if (home.code !== 0 || listing.code !== 0) return warn(`could not list processes in ${vm}, so a stray Chrome stays open`)
  const pids = strayChromePids(listing.stdout, `${home.stdout.trim()}/qa-chrome-profile`)
  if (pids.length > 0) tartExec(vm, `kill ${pids.join(' ')}`)
}

// Tips notifications cover the screen in recordings. launchd starts the agent again after a reboot.
function stopTips(vm: string): void {
  const stopped = tartExec(vm, 'launchctl bootout gui/$(id -u)/com.apple.tipsd 2>/dev/null; launchctl disable gui/$(id -u)/com.apple.tipsd')
  if (stopped.code !== 0) warn(`could not stop the Tips agent in ${vm}, so Tips notifications can cover recordings`)
}

// Everything a freshly booted guest needs, shared by up and restart.
function prepareGuest(vm: string, options: {freshClone: boolean}, profile: Profile): void {
  tartExec(vm, `mkdir -p ${PULL_DIR}`)
  stopTips(vm)
  rewriteReplaydApproval(vm)
  ensureLoopbackMic(vm)
  ensureDisplayMode(vm)
  ensureCliclick(vm)
  quitStrayChrome(vm)
  if (startChrome(vm, profile.chrome.mediaOrigins)) {
    console.log(`${vm} ready, Chrome on port ${CHROME_PORT}`)
    reportGoogleSession(vm, {record: options.freshClone})
  } else console.error(`${vm} ready. WARNING: Chrome did not open port ${CHROME_PORT}, so qa-browser will fail`)
  reportImage(vm)
  reportApprovalAge()
}

function reportApprovalAge(): void {
  try {
    const warning = approvalWarning(readHostImageState().steps.approve, new Date())
    if (warning) console.error(warning)
  } catch (error) {
    warn(`could not check the screen-capture approval age: ${error instanceof Error ? error.message : String(error)}`)
  }
}

// The base image is not pinned: its build runs brew upgrade, and up installs missing tools.
// The versions that ran go to the log and the local summary, so two runs can be compared.
function reportImage(vm: string): void {
  mkdirSync(PULL_DIR, {recursive: true})
  const query = tartExec(
    vm,
    'print -r -- "macOS $(sw_vers -productVersion), Chrome $(defaults read "/Applications/Google Chrome.app/Contents/Info" CFBundleShortVersionString), node $(node --version), bun $(bun --version), cliclick $(cliclick -V 2>&1 | grep -oE "[0-9][0-9.]*" | head -1), image provisioned $(cat ~/.qa-image-provisioned 2>/dev/null || print unknown)"'
  )
  const line = query.code === 0 ? query.stdout.trim() : 'unknown: the version query failed'
  writeFileSync(join(PULL_DIR, `${vm}-image.txt`), `${line}\n`)
  console.log(`image: ${line}`)
}

function up(pr: number, worktree: string, window: boolean, env: Env, profile: Profile): void {
  const vm = vmName(pr)
  if (!existsSync(worktree)) throw new VmError(`no worktree at ${worktree}`, EXIT.usage)
  if (baseImageState(run(['tart', 'get', BASE_IMAGE, '--format', 'json'])) === 'running')
    throw new VmError(`${BASE_IMAGE} is running (a vm image step is using it); retry when it stops`, EXIT.inUse)
  try {
    prepareLease(vm, env)
  } catch (error) {
    if (error instanceof VmError) throw error
    throw new VmError(`could not check who owns ${vm}, so it was not replaced: ${error instanceof Error ? error.message : String(error)}`, EXIT.unavailable)
  }
  run(['tart', 'delete', vm])
  const cloned = run(['tart', 'clone', BASE_IMAGE, vm], {stderr: 'inherit'})
  if (cloned.code !== 0) throw new VmError(`tart clone ${BASE_IMAGE} ${vm} failed`, EXIT.unavailable)
  try {
    bootVm(vm, {window, softnet: true})
  } catch (error) {
    run(['tart', 'delete', vm])
    releaseLease(vm)
    throw error
  }
  copyCheckout(vm, worktree, profile.worktree.paths, '~/wt')
  copyVideoTools(vm, worktree, profile.fonts)
  installInputShims(vm, profile.inputShims)
  console.log(`${vm} up, worktree copied to ~/wt`)
  prepareGuest(vm, {freshClone: true}, profile)
}

function readOwnershipOrRefuse(vm: string, env: Env): ReturnType<typeof readOwnership> {
  try {
    return readOwnership(env)
  } catch (error) {
    throw new VmError(`could not check who owns ${vm}, so it was left alone: ${error instanceof Error ? error.message : String(error)}`, EXIT.unavailable)
  }
}

function restart(pr: number, window: boolean, env: Env, profile: Profile): void {
  const vm = vmName(pr)
  const {vms, leases, table, owner} = readOwnershipOrRefuse(vm, env)
  const plan = planRestart(vm, vms, leases, table, owner)
  if (plan.kind === 'missing') throw new VmError(`${vm} does not exist; run vm up ${pr} <worktree>`, EXIT.noInput)
  if (plan.kind === 'refused') throw new VmError(`refusing ${vm}: ${plan.reason}`, EXIT.inUse)
  takeLease(vm, owner)
  tartExec(vm, 'sync')
  const stop = run(['tart', 'stop', vm])
  const stopped = confirmStopped(vm, tartVms(), stop)
  if (!stopped.ok) throw new VmError(`could not restart ${vm}: ${stopped.reason}`, EXIT.unavailable)
  bootVm(vm, {window, softnet: true})
  console.log(`${vm} restarted, ~/wt kept`)
  prepareGuest(vm, {freshClone: false}, profile)
}

function down(pr: number): void {
  const vm = vmName(pr)
  if (!succeeds(['tart', 'get', vm])) {
    releaseLease(vm)
    console.log(`${vm} does not exist`)
    return
  }
  run(['tart', 'stop', vm])
  const deleted = run(['tart', 'delete', vm])
  if (deleted.code !== 0) throw new VmError(`tart delete ${vm} failed: ${deleted.stderr.trim()}`, 1)
  releaseLease(vm)
  console.log(`${vm} deleted`)
}

function pull(pr: number, path: string, to: string | undefined): void {
  const vm = vmName(pr)
  const directory = to ?? PULL_DIR
  mkdirSync(directory, {recursive: true})
  const target = to === undefined ? join(PULL_DIR, `${vm}-${basename(path)}`) : join(to, basename(path))
  const copied = run(['tart', 'exec', vm, 'cat', path], {stdout: {file: target}})
  if (copied.code !== 0) throw new VmError(`could not read ${path} in ${vm}: ${copied.stderr.trim()}`, EXIT.noInput)
  console.log(target)
}

// The proxy answers the MCP client on stdio, so this process stays attached until the client closes it.
async function browser(pr: number): Promise<number> {
  const child = spawn(
    'tart',
    [
      'exec',
      '-i',
      vmName(pr),
      'zsh',
      '-lc',
      `QA_OS_INPUT=1 QA_CDP_PORT=${CHROME_PORT} ${VM_BUN} ${VM_VIDEO_TOOLS}/proxy.ts npx -y @playwright/mcp@latest --cdp-endpoint http://127.0.0.1:${CHROME_PORT} --output-dir ${PULL_DIR} 2>/tmp/qa-browser-mcp.log`
    ],
    {stdio: 'inherit'}
  )
  const forward = (signal: NodeJS.Signals) => () => child.kill(signal)
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP'] as const) process.on(signal, forward(signal))
  return new Promise(resolve => child.on('exit', code => resolve(code ?? 1)))
}

const RecorderPaths = z.object({
  recording: z.string(),
  timeline: z.string(),
  movies: z.array(z.string()),
  logs: z.array(z.string()).default([]),
  step: z.union([z.string(), z.number()]).optional()
})
const RecordingStep = z.object({step: z.union([z.string(), z.number()])})
const JsonText = z.string().transform((text, context) => {
  try {
    return JSON.parse(text) as unknown
  } catch {
    context.addIssue({code: z.ZodIssueCode.custom, message: 'not JSON'})
    return z.NEVER
  }
})
const stepOf = (file: string) => String(JsonText.pipe(RecordingStep).parse(readFileSync(file, 'utf8')).step)

function videoDir(vm: string): string {
  return join(PULL_DIR, `${vm}-video`)
}

// Each step keeps its own sources on the host, because the VM's next record start overwrites
// recording.json and timeline.jsonl. Without them a step's video cannot be edited again.
function recordStop(pr: number): number {
  const vm = vmName(pr)
  const stopped = run(['tart', 'exec', vm, VM_BUN, `${VM_VIDEO_TOOLS}/cli.ts`, 'stop'], {stderr: 'inherit'})
  if (stopped.code !== 0) throw stopFailure(vm, stopped.code)
  const paths = JsonText.pipe(RecorderPaths).safeParse(stopped.stdout)
  if (!paths.success) throw new VmError(`the recorder in ${vm} printed an unexpected result: ${stopped.stdout.trim()}`, 1)
  const work = join(videoDir(vm), 'incoming')
  rmSync(work, {recursive: true, force: true})
  mkdirSync(work, {recursive: true})
  const fetch = (from: string, to: string) => {
    const copied = run(['tart', 'exec', vm, 'cat', from], {stdout: {file: join(work, to)}})
    if (copied.code !== 0) throw new VmError(`could not copy ${from} from ${vm}`, EXIT.noInput)
  }
  fetch(paths.data.recording, 'recording.json')
  fetch(paths.data.timeline, 'timeline.jsonl')
  paths.data.movies.forEach((movie, index) => fetch(movie, `segment-${index + 1}.mov`))
  paths.data.logs.forEach((log, index) => fetch(log, `stop-${index + 1}.log`))
  const step = stepOf(join(work, 'recording.json'))
  const kept = join(videoDir(vm), `step-${step}`)
  rmSync(kept, {recursive: true, force: true})
  renameSync(work, kept)
  if (paths.data.logs.length > 0) console.error(`capture stop logs (cause still unknown): ${kept}/stop-*.log`)
  const removed = run(['tart', 'exec', vm, 'rm', '-f', ...paths.data.movies, ...paths.data.logs])
  if (removed.code !== 0) warn(`could not delete the movie files in ${vm}: ${removed.stderr.trim()}`)
  return renderStep(vm, kept)
}

function renderStep(vm: string, dir: string): number {
  const recording = join(dir, 'recording.json')
  if (!existsSync(recording)) throw new VmError(`no kept sources in ${dir}`, EXIT.noInput)
  const movies = readdirSync(dir)
    .filter(file => /^segment-\d+\.mov$/.test(file))
    .sort((left, right) => Number(left.match(/\d+/)?.[0]) - Number(right.match(/\d+/)?.[0]))
    .map(file => join(dir, file))
  const step = stepOf(recording)
  return run([process.execPath, join(VIDEO_DIR, 'render.ts'), recording, join(dir, 'timeline.jsonl'), FONTS_DIR, join(PULL_DIR, `${vm}-step-${step}.mp4`), ...movies], {
    stdout: 'inherit',
    stderr: 'inherit'
  }).code
}

function recordMark(pr: number, mark: MarkKind, text: string): number {
  const vm = vmName(pr)
  const cli = [VM_BUN, `${VM_VIDEO_TOOLS}/cli.ts`, 'mark']
  return mark === 'key'
    ? run(['tart', 'exec', vm, ...cli, 'key', text], {stdout: 'inherit', stderr: 'inherit'}).code
    : run(['tart', 'exec', '-i', vm, ...cli, 'note'], {stdin: text, stdout: 'inherit', stderr: 'inherit'}).code
}

function list(): void {
  const rows = describeVms(tartVms(), readLeases(), processTable())
  if (rows.length === 0) return console.log('no QA VMs')
  const width = Math.max(...rows.map(row => row.vm.length))
  rows.forEach(row => console.log(`${row.vm.padEnd(width)}  ${row.state.padEnd(7)}  owner ${row.owner}`))
}

function reportNetworkProblem(pr: number): void {
  const probe = tartExec(vmName(pr), PROBE_SCRIPT)
  if (probe.code !== 0) return
  const parsed = parseProbe(probe.stdout)
  const diagnosis = parsed === undefined ? undefined : diagnoseNetwork(pr, parsed)
  if (diagnosis !== undefined) console.error(diagnosis)
}

// Only the commands that set up a VM or the image read the profile, so a bad profile never breaks exec or record.
export async function execute(command: Exclude<Command, {kind: 'doctor'}>, env: Env, readProfile: () => Profile): Promise<number> {
  const inherit = {stdout: 'inherit', stderr: 'inherit'} as const
  switch (command.kind) {
    case 'up':
      up(command.pr, command.worktree, command.window, env, readProfile())
      return 0
    case 'exec': {
      const piped = hostStdinCarriesData()
      const interactive = piped ? ['-i'] : []
      const result = await runExecWithRetry(['tart', 'exec', ...interactive, vmName(command.pr), 'zsh', '-lc', `cd ~/wt && ${command.commandLine}`], piped)
      if (shouldDiagnoseNetwork(result)) reportNetworkProblem(command.pr)
      return result.code
    }
    case 'pull':
      pull(command.pr, command.path, command.to)
      return 0
    case 'record-start':
      return run(['tart', 'exec', '-i', vmName(command.pr), VM_BUN, `${VM_VIDEO_TOOLS}/cli.ts`, 'start', command.step], {stdin: command.caption, ...inherit}).code
    case 'record-stop':
      return recordStop(command.pr)
    case 'record-rerender':
      return renderStep(vmName(command.pr), join(videoDir(vmName(command.pr)), `step-${command.step}`))
    case 'record-mark':
      return recordMark(command.pr, command.mark, command.text)
    case 'record-pause':
      return run(['tart', 'exec', '-i', vmName(command.pr), VM_BUN, `${VM_VIDEO_TOOLS}/cli.ts`, 'pause'], {stdin: command.reason, ...inherit}).code
    case 'record-resume':
      return run(['tart', 'exec', vmName(command.pr), VM_BUN, `${VM_VIDEO_TOOLS}/cli.ts`, 'resume'], inherit).code
    case 'restart':
      restart(command.pr, command.window, env, readProfile())
      return 0
    case 'down':
      down(command.pr)
      return 0
    case 'browser':
      return browser(command.pr)
    case 'list':
      list()
      return 0
    case 'image-build':
      await imageBuild(command.checkout, readProfile())
      return 0
    case 'image-provision': {
      const files = readProvisionFiles(readProfile().provision.files)
      await withBaseImage({window: false}, vm => provisionStep(vm, files))
      console.log(`provisioned ${BASE_IMAGE}. Next: vm image grant`)
      return 0
    }
    case 'image-grant': {
      const origins = readProfile().chrome.mediaOrigins
      await withBaseImage({window: false}, vm => grantStep(vm, origins))
      return 0
    }
    case 'image-approve': {
      const origins = readProfile().chrome.mediaOrigins
      await withBaseImage({window: true}, vm => approveStep(vm, origins))
      return 0
    }
    case 'image-signin': {
      const profile = readProfile()
      await withBaseImage({window: true}, vm => signinStep(vm, profile))
      return 0
    }
    case 'image-warm': {
      const profile = readProfile()
      if (profile.warm === undefined) throw new VmError('the vm profile defines no warm step, so there is nothing to warm', EXIT.config)
      const plan = planWarm(command.checkout, profile.warm)
      await withBaseImage({window: false}, vm => warmStep(vm, plan, profile.worktree.paths))
      console.log(`warmed ${BASE_IMAGE}`)
      return 0
    }
    case 'session-inject-electron': {
      const stored = loadSession()
      if (stored.kind === 'missing') throw new VmError('no stored Google session: run vm image signin', EXIT.noInput)
      if (stored.kind === 'invalid') throw new VmError(`stored Google session not usable: ${stored.reason}. Run vm image signin`, EXIT.noInput)
      copyGuestTools(vmName(command.pr))
      importCookies(vmName(command.pr), stored.record.cookies, {port: command.port, url: command.url})
      console.log(`wrote ${stored.record.cookies.length} Google cookies into the Electron app on port ${command.port}`)
      return 0
    }
  }
}

