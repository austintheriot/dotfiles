import {existsSync} from 'node:fs'
import {cliclickPlan, keystrokeScript, systemEventsKeystroke, type OsAction} from './os-input'

const CLICLICK = '/opt/homebrew/bin/cliclick'
// A pending TCC (privacy permission) prompt blocks osascript until someone answers it. A killed run has no
// exit code, so it counts as a failure and the caller falls back to CDP.
const OSASCRIPT_TIMEOUT_MS = 5000
const ACTIVATE = (pid: number) =>
  `ObjC.import("AppKit"); const app = $.NSRunningApplication.runningApplicationWithProcessIdentifier(${pid}); ` +
  'app.isNil() ? "none" : (app.activateWithOptions($.NSApplicationActivateIgnoringOtherApps), "ok")'

function listenerPid(port: number): number | undefined {
  const output = Bun.spawnSync(['/usr/sbin/lsof', '-nP', `-iTCP:${port}`, '-sTCP:LISTEN', '-t']).stdout.toString()
  const pid = Number(output.trim().split('\n')[0])
  return Number.isInteger(pid) && pid > 0 ? pid : undefined
}

export function bringToFront(port: number): boolean {
  const pid = listenerPid(port)
  if (pid === undefined) return false
  const result = Bun.spawnSync(['/usr/bin/osascript', '-l', 'JavaScript', '-e', ACTIVATE(pid)], {timeout: OSASCRIPT_TIMEOUT_MS})
  return result.stdout.toString().trim() === 'ok'
}

// Real input moves the mouse of the machine it runs on, so only the QA VM turns it on.
export const osInputEnabled = () => process.env['QA_OS_INPUT'] === '1' && existsSync(CLICLICK)

const cliclick = (args: string[]) => args.length === 0 || Bun.spawnSync([CLICLICK, ...args], {stdout: 'ignore', stderr: 'inherit'}).exitCode === 0

const systemEvents = (script: string) =>
  Bun.spawnSync(['/usr/bin/osascript', '-e', script], {stdout: 'ignore', stderr: 'inherit', timeout: OSASCRIPT_TIMEOUT_MS}).exitCode === 0

// Returns the time the input itself started, after the pointer glided to the target.
export function performOs(action: OsAction): number | undefined {
  const keystroke = action.kind === 'key' ? systemEventsKeystroke(action.key) : undefined
  if (keystroke) return osInputEnabled() && systemEvents(keystrokeScript(keystroke)) ? Date.now() / 1000 : undefined
  const plan = cliclickPlan(action)
  if (plan === undefined || !osInputEnabled() || !cliclick(plan.glide)) return undefined
  const actedAt = Date.now() / 1000
  return cliclick(plan.act) ? actedAt : undefined
}
