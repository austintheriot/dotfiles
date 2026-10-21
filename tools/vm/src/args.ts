import {Argument, Command as Program, CommanderError, InvalidArgumentError} from '@commander-js/extra-typings'
import manifest from '../package.json'
import {ELECTRON_PORT} from './config'
import {NOTE_MAX, textLength} from '../video/core'

export const VERSION = manifest.version

export type Pr = number
export type MarkKind = 'key' | 'note'
export type Command =
  | {kind: 'up'; pr: Pr; worktree: string; window: boolean}
  | {kind: 'exec'; pr: Pr; commandLine: string}
  | {kind: 'pull'; pr: Pr; path: string; to: string | undefined}
  | {kind: 'record-start'; pr: Pr; step: string; caption: string}
  | {kind: 'record-stop'; pr: Pr}
  | {kind: 'record-rerender'; pr: Pr; step: string}
  | {kind: 'record-mark'; pr: Pr; mark: MarkKind; text: string}
  | {kind: 'record-pause'; pr: Pr; reason: string}
  | {kind: 'record-resume'; pr: Pr}
  | {kind: 'restart'; pr: Pr; window: boolean}
  | {kind: 'down'; pr: Pr}
  | {kind: 'browser'; pr: Pr}
  | {kind: 'list'}
  | {kind: 'image-build'; checkout: string | undefined}
  | {kind: 'image-provision'}
  | {kind: 'image-warm'; checkout: string}
  | {kind: 'image-grant'}
  | {kind: 'image-approve'}
  | {kind: 'image-signin'}
  | {kind: 'session-inject-electron'; pr: Pr; port: number; url: string | undefined}
  | {kind: 'doctor'}

export type Parsed = {kind: 'run'; command: Command} | {kind: 'exit'; code: number; stdout: string; stderr: string}

export const EX_USAGE = 64

function parsePr(value: string): Pr {
  if (!/^\d+$/.test(value)) throw new InvalidArgumentError(`PR number must be digits, got '${value}'`)
  return Number(value)
}

function parsePort(value: string): number {
  const port = Number(value)
  if (!/^\d+$/.test(value) || port < 1 || port > 65535) throw new InvalidArgumentError(`port must be 1 to 65535, got '${value}'`)
  return port
}

function parseStep(value: string): string {
  if (!/^[A-Za-z0-9-]+$/.test(value)) throw new InvalidArgumentError(`step must be letters, digits, or dashes, got '${value}'`)
  return value
}

// The file lands on the host as /tmp/pr-review-qa/<vm>-<name>, so the name must be plain.
function parsePullPath(value: string): string {
  const name = value.split('/').at(-1) ?? ''
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(name)) throw new InvalidArgumentError(`refusing file name '${name}'`)
  return value
}

export function parseMarkText(value: string): string {
  const length = textLength(value)
  if (length > NOTE_MAX) throw new InvalidArgumentError(`text is ${length} characters, and the video chip shows at most ${NOTE_MAX}`)
  return value
}

const prArgument = () => new Argument('<pr>', 'pull request number; the VM is pr-qa-<pr>').argParser(parsePr)

type Output = {writeOut: (text: string) => void; writeErr: (text: string) => void}

// Subcommands copy exitOverride and the output settings from their parent when they are created.
function buildProgram(env: Record<string, string | undefined>, output: Output, emit: (command: Command) => void) {
  const program = new Program('vm')
    .description('Per-PR macOS QA VMs: boot one, run commands in it, record Dev QA steps, and delete it.')
    .version(VERSION)
    .exitOverride()
    .configureOutput(output)
    .showHelpAfterError()
    .enablePositionalOptions()

  program
    .command('up')
    .description('clone pr-qa-base, boot it, and copy the worktree paths the profile names into ~/wt')
    .addArgument(prArgument())
    .argument('<worktree>', 'a checkout of the PR head on the host')
    .option('--headless', 'boot without a VM window (also PR_REVIEW_VM_WINDOW=0)')
    .addHelpText('after', '\nExit codes: 73 another live session owns the VM, 75 the host is at its VM limit (retry later), 69 the VM did not boot.')
    .action((pr, worktree, options) => emit({kind: 'up', pr, worktree, window: !options.headless && env['PR_REVIEW_VM_WINDOW'] !== '0'}))

  program
    .command('exec')
    .description('run one zsh command line in ~/wt inside the VM')
    .addArgument(prArgument())
    .argument('<command-line...>', 'the command line; its words are joined with spaces')
    .passThroughOptions()
    .allowUnknownOption()
    .action((pr, words) => emit({kind: 'exec', pr, commandLine: words.join(' ')}))

  program
    .command('pull')
    .description('copy one VM file to the host: /tmp/pr-review-qa/pr-qa-<pr>-<name> by default')
    .addArgument(prArgument())
    .argument('<vm-path>', 'the file in the VM', parsePullPath)
    .option('--to <dir>', 'write <dir>/<name> instead, for example a QA unit directory')
    .action((pr, path, options) => emit({kind: 'pull', pr, path, to: options.to}))

  const record = program.command('record').description('record Dev QA steps as edited MP4 videos')
  record
    .command('start')
    .description('start a full-screen recording of one Dev QA step')
    .addArgument(prArgument())
    .argument('<step>', 'the step name: letters, digits, or dashes', parseStep)
    .argument('<caption>', 'the caption the video shows')
    .action((pr, step, caption) => emit({kind: 'record-start', pr, step, caption}))
  record
    .command('stop')
    .description('stop the recording and edit it into /tmp/pr-review-qa/pr-qa-<pr>-step-<step>.mp4')
    .addArgument(prArgument())
    .action(pr => emit({kind: 'record-stop', pr}))
  record
    .command('rerender')
    .description('edit a stopped step again from its kept sources')
    .addArgument(prArgument())
    .argument('<step>', 'the step name', parseStep)
    .action((pr, step) => emit({kind: 'record-rerender', pr, step}))
  record
    .command('mark')
    .description('show a key press (for example Meta+n) or a described action in the video')
    .addArgument(prArgument())
    .addArgument(new Argument('<kind>', 'what to show').choices(['key', 'note'] as const))
    .argument('<text>', `the key, or the action to describe (at most ${NOTE_MAX} characters)`, parseMarkText)
    .action((pr, mark, text) => emit({kind: 'record-mark', pr, mark, text}))
  record
    .command('pause')
    .description('stop capturing the screen until resume, while a person does a step by hand (a sign-in, a consent screen)')
    .addArgument(prArgument())
    .argument('[reason]', `what the person does; the video shows it on a dark card (at most ${NOTE_MAX} characters)`, parseMarkText, 'human sign-in')
    .action((pr, reason) => emit({kind: 'record-pause', pr, reason}))
  record
    .command('resume')
    .description('start capturing again after pause')
    .addArgument(prArgument())
    .action(pr => emit({kind: 'record-resume', pr}))

  const image = program.command('image').description('build and maintain pr-qa-base, the image every QA VM clones')
  image
    .command('build')
    .description('run only the image steps that are due: provision, grant, approve, warm, signin')
    .option('--checkout <path>', 'a checkout of the profile\'s project on the host; without it, warm is skipped')
    .addHelpText('after', '\nNeeds a terminal. Steps run in order, and provision reruns every later step.')
    .action((options: {checkout?: string}) => emit({kind: 'image-build', checkout: options.checkout}))
  image
    .command('provision')
    .description('install the toolchain and the profile\'s provision files in pr-qa-base')
    .addHelpText('after', '\nExit codes: 66 pr-qa-base or a provision file is missing, 73 pr-qa-base is running, 75 the host is at its VM limit, 69 a guest step failed.')
    .action(() => emit({kind: 'image-provision'}))
  image
    .command('grant')
    .description('grant every macOS permission QA needs in pr-qa-base: privacy (TCC), Chrome policy, and prompts off')
    .addHelpText('after', '\nNeeds SIP off in the guest. Rerun after a tart-guest-agent upgrade.')
    .action(() => emit({kind: 'image-grant'}))
  image
    .command('approve')
    .description('click the macOS prompts no script can answer in pr-qa-base, in its VM window')
    .addHelpText('after', '\nNeeds a terminal. Run it after vm image grant.')
    .action(() => emit({kind: 'image-approve'}))
  image
    .command('signin')
    .description('restore or capture the Google session in pr-qa-base\'s Chrome, then store it sealed on the host')
    .addHelpText('after', '\nNeeds a terminal for the guided sign-in. Run it after vm image approve.')
    .action(() => emit({kind: 'image-signin'}))
  image
    .command('warm')
    .description('fill the package caches in pr-qa-base from a checkout, with the profile\'s warm installs, so PR VMs install without registry secrets')
    .argument('<checkout>', 'a checkout of the profile\'s project on the host')
    .action(checkout => emit({kind: 'image-warm', checkout}))

  const session = program.command('session').description('use the stored Google session outside pr-qa-base')
  session
    .command('inject-electron')
    .description('write the stored Google cookies into an Electron app in the PR VM, so an OAuth2 Proxy page passes without a password')
    .addArgument(prArgument())
    .option('--port <port>', 'the Electron app\'s CDP port', parsePort, ELECTRON_PORT)
    .option('--url <substring>', 'pick the shell page whose URL contains this')
    .action((pr, options) => emit({kind: 'session-inject-electron', pr, port: options.port, url: options.url}))

  program
    .command('restart')
    .description('stop and boot the VM again, keeping ~/wt and its builds; use it when the network breaks after the host sleeps (Softnet) or after a guest reboot')
    .addArgument(prArgument())
    .option('--headless', 'boot without a VM window (also PR_REVIEW_VM_WINDOW=0)')
    .addHelpText(
      'after',
      '\nExit codes: 66 the VM does not exist, 73 another live session owns the VM, 75 the host is at its VM limit (retry later), 69 the VM did not stop or did not boot.'
    )
    .action((pr, options) => emit({kind: 'restart', pr, window: !options.headless && env['PR_REVIEW_VM_WINDOW'] !== '0'}))

  program
    .command('down')
    .description('stop and delete the VM')
    .addArgument(prArgument())
    .action(pr => emit({kind: 'down', pr}))

  program
    .command('browser')
    .description('stdio Playwright MCP server attached to the VM\'s Chrome (for an MCP config)')
    .addArgument(prArgument())
    .action(pr => emit({kind: 'browser', pr}))

  program
    .command('list')
    .alias('ls')
    .description('list the QA VMs and the session that owns each one')
    .action(() => emit({kind: 'list'}))

  program
    .command('doctor')
    .description('check that this Mac can run QA VMs, and say how to fix what is missing')
    .action(() => emit({kind: 'doctor'}))

  return program
}

export async function parseArgs(argv: string[], env: Record<string, string | undefined>): Promise<Parsed> {
  let stdout = ''
  let stderr = ''
  let command: Command | undefined
  const output = {writeOut: (text: string) => void (stdout += text), writeErr: (text: string) => void (stderr += text)}
  const program = buildProgram(env, output, parsed => (command = parsed))
  try {
    if (argv.length === 0) {
      program.outputHelp()
      return {kind: 'exit', code: 0, stdout, stderr}
    }
    await program.parseAsync(argv, {from: 'user'})
  } catch (error) {
    if (!(error instanceof CommanderError)) throw error
    const informational = ['commander.helpDisplayed', 'commander.version', 'commander.help'].includes(error.code)
    return {kind: 'exit', code: informational ? 0 : EX_USAGE, stdout, stderr}
  }
  return command ? {kind: 'run', command} : {kind: 'exit', code: EX_USAGE, stdout, stderr}
}
