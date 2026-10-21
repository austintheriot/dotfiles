import {quitChrome, startChrome} from '../chrome'
import {EXIT} from '../config'
import {tartExec, VmError, warn} from '../proc'
import {readMarkers, writeMarker} from './base-image'
import {agentPath, LOCAL_NETWORK_SCRIPTED, REPLAYD_SCRIPTED} from './grant'
import {lastAlertedScript} from './grants'
import {isCurrentAge, markerAgeDays} from './plan'

export function remainingPrompts(scripted: {replayd: boolean; localNetwork: boolean}): string[] {
  return [
    ...(scripted.replayd ? [] : ['"tart-guest-agent" is requesting to bypass the system private window picker: click Allow.']),
    ...(scripted.localNetwork ? [] : ['"Google Chrome" may find devices on local networks: click Allow.'])
  ]
}

export function terminalRefusal(step: 'approve' | 'signin', stdinIsTerminal: boolean): VmError | undefined {
  return stdinIsTerminal ? undefined : new VmError(`needs a terminal: run vm image ${step} yourself`, EXIT.unavailable)
}

export const isConfirmed = (answer: string | undefined): boolean => ['y', 'yes'].includes((answer ?? '').trim().toLowerCase())

export function parseLastAlerted(output: string): Date | undefined {
  const text = output.trim()
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(text)) return undefined
  const date = new Date(text)
  return Number.isNaN(date.getTime()) ? undefined : date
}

// The local network answer does not lapse, so any earlier approve marker means it was given.
export function planApprove(facts: {
  scripted: {replayd: boolean; localNetwork: boolean}
  lastAlerted: Date | undefined
  approvedBefore: boolean
  now: Date
}): {prompts: string[]} {
  const current = facts.lastAlerted !== undefined && isCurrentAge(markerAgeDays(facts.lastAlerted.toISOString(), facts.now))
  return {prompts: remainingPrompts({replayd: facts.scripted.replayd || current, localNetwork: facts.scripted.localNetwork || facts.approvedBefore})}
}

async function readLine(prompt: string): Promise<string | undefined> {
  process.stdout.write(prompt)
  for await (const line of console) return line
  return undefined
}

export async function waitForEnter(prompt: string): Promise<void> {
  await readLine(prompt)
}

export const askConfirmation = async (prompt: string): Promise<boolean> => isConfirmed(await readLine(prompt))

function readLastAlerted(vm: string, agent: string): Date | undefined {
  const read = tartExec(vm, lastAlertedScript(agent))
  return read.code === 0 ? parseLastAlerted(read.stdout) : undefined
}

function recordApproval(vm: string, agent: string, chrome: string): void {
  const lastAlerted = readLastAlerted(vm, agent)
  if (lastAlerted === undefined) warn(`could not read the screen-capture approval date in ${vm}; recording today, so the 28-day warning can come late`)
  writeMarker(vm, 'approve', {chrome}, lastAlerted ?? new Date())
}

export async function approveStep(vm: string, mediaOrigins: readonly string[]): Promise<void> {
  const agent = agentPath(vm)
  const chrome = tartExec(vm, 'defaults read "/Applications/Google Chrome.app/Contents/Info" CFBundleShortVersionString').stdout.trim()
  const {prompts} = planApprove({
    scripted: {replayd: REPLAYD_SCRIPTED, localNetwork: LOCAL_NETWORK_SCRIPTED},
    lastAlerted: readLastAlerted(vm, agent),
    approvedBefore: readMarkers(vm).approve !== undefined,
    now: new Date()
  })
  if (prompts.length === 0) {
    console.log('no macOS prompt needs a click: the screen-capture approval is current, and the local network prompt was answered before')
    return recordApproval(vm, agent, chrome)
  }
  const refusal = terminalRefusal('approve', process.stdin.isTTY === true)
  if (refusal) throw refusal
  quitChrome(vm)
  const needsCapture = prompts.some(prompt => prompt.includes('tart-guest-agent'))
  if (needsCapture) tartExec(vm, '(screencapture -v -V 120 -x /tmp/approve.mov &>/dev/null &)')
  startChrome(vm, mediaOrigins)
  console.log(['In the VM window, answer each dialog that appears:', ...prompts.map((prompt, index) => `  ${index + 1}. ${prompt}`)].join('\n'))
  await waitForEnter('Press Enter here after you answered them... ')
  const confirmed = await askConfirmation('Did you click Allow on every prompt? [y/N] ')
  tartExec(vm, 'pkill -INT -x screencapture; rm -f /tmp/approve.mov; true')
  if (!quitChrome(vm)) throw new VmError(`Chrome in ${vm} did not quit, so its answers may not be saved`, EXIT.unavailable)
  if (!confirmed) throw new VmError('approve not confirmed; rerun vm image approve', EXIT.unavailable)
  recordApproval(vm, agent, chrome)
}
