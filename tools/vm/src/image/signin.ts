import {userInfo} from 'node:os'
import {quitChrome, startChrome} from '../chrome'
import {CHROME_PORT, EXIT, SESSION_KEY_SERVICE} from '../config'
import type {Profile} from '../profile'
import {VmError, warn} from '../proc'
import {terminalRefusal, waitForEnter} from './approve'
import {readHostImageState, updateHostImageState, writeMarker} from './base-image'
import {copyGuestTools, exportCookies, importCookies, openTab, probeGoogle} from './guest-tools'
import {selectSessionCookies, type GoogleSessionState, type KeepCookie} from './session'
import {loadSession, saveSession, type StoredSession} from './session-store'

// Spike S1 (2026-10-15): Google read the restored cookies as signed out at once, so signin always guides.
export const RESTORE_ENABLED = false

export function planSignin(stored: StoredSession, restoreEnabled: boolean): {restore: boolean; warning: string | undefined} {
  if (stored.kind === 'invalid') return {restore: false, warning: `stored Google session not used: ${stored.reason}`}
  return {restore: stored.kind === 'found' && restoreEnabled, warning: undefined}
}

export function expiryMessage(
  state: GoogleSessionState,
  context: {vm: string; previous: GoogleSessionState | undefined}
): {level: 'warn' | 'info'; text: string} | undefined {
  if (state === 'signed-out' && context.previous === 'signed-in')
    return {level: 'warn', text: `Google session in ${context.vm} expired: it was signed in at the last check. Run vm image signin.`}
  if (state === 'signed-out') return {level: 'warn', text: `Google session in ${context.vm} is signed out. Run vm image signin.`}
  if (state === 'unknown') return {level: 'warn', text: `could not tell whether the Google session in ${context.vm} is signed in`}
  return undefined
}

const causeOf = (error: unknown): string => (error instanceof Error ? error.message : 'unknown error')

function recordGoogleState(state: GoogleSessionState): void {
  updateHostImageState(current => ({...current, google: {state, at: new Date().toISOString()}}))
}

function exportSession(vm: string, keep: readonly KeepCookie[]): void {
  const cookies = selectSessionCookies(exportCookies(vm), keep)
  if (cookies.length === 0) throw new VmError(`Chrome in ${vm} holds no Google cookies to store`, EXIT.unavailable)
  saveSession({version: 1, exportedAt: new Date().toISOString(), account: userInfo().username, cookies})
  console.log(`stored ${cookies.length} Google cookies, sealed under the Keychain key`)
}

function loadStoredSession(): StoredSession {
  try {
    return loadSession()
  } catch (error) {
    if (error instanceof VmError) throw error
    throw new VmError(`could not read the stored Google session or its Keychain item ${SESSION_KEY_SERVICE}: ${causeOf(error)}`, EXIT.unavailable)
  }
}

const GOOGLE_SIGN_IN = 'https://accounts.google.com'

export function signinGuide(signin: Profile['signin']): {tabs: string[]; lines: string[]; prompt: string} {
  const steps = ['Sign in to Google with your account in the accounts.google.com tab.', ...signin.steps]
  return {
    tabs: [GOOGLE_SIGN_IN, ...signin.tabs],
    lines: ['In the VM window:', ...steps.map((step, index) => `  ${index + 1}. ${step}`)],
    prompt: signin.tabs.length === 0 ? 'Press Enter here when you are signed in... ' : 'Press Enter here when every tab has loaded... '
  }
}

export async function signinStep(vm: string, profile: Profile): Promise<void> {
  const keep = profile.session.keepCookies
  const stored = loadStoredSession()
  copyGuestTools(vm)
  quitChrome(vm)
  if (!startChrome(vm, profile.chrome.mediaOrigins)) throw new VmError(`Chrome in ${vm} did not open its debugging port`, EXIT.unavailable)
  const plan = planSignin(stored, RESTORE_ENABLED)
  if (plan.warning) warn(plan.warning)
  if (plan.restore && stored.kind === 'found') {
    importCookies(vm, stored.record.cookies, {port: CHROME_PORT, url: undefined})
    if (probeGoogle(vm) === 'signed-in') return finish(vm, 'restored', keep)
    console.log('the stored session no longer signs in; falling back to a guided sign-in')
  } else if (probeGoogle(vm) === 'signed-in') {
    return finish(vm, 'already signed in', keep)
  }
  const refusal = terminalRefusal('signin', process.stdin.isTTY === true)
  if (refusal) throw refusal
  const guide = signinGuide(profile.signin)
  guide.tabs.forEach(tab => openTab(vm, tab))
  console.log(guide.lines.join('\n'))
  await waitForEnter(guide.prompt)
  if (probeGoogle(vm) !== 'signed-in') throw new VmError(`Google still reports signed out in ${vm}; run vm image signin again`, EXIT.unavailable)
  finish(vm, 'signed in', keep)
}

function finish(vm: string, how: string, keep: readonly KeepCookie[]): void {
  exportSession(vm, keep)
  if (!quitChrome(vm)) warn(`Chrome in ${vm} did not quit, so the profile on disk can miss the newest cookies`)
  recordGoogleState('signed-in')
  writeMarker(vm, 'signin', {})
  console.log(`Google session in ${vm}: ${how}`)
}

// Only a fresh clone is a copy of pr-qa-base. A restarted clone can be signed out by QA itself,
// so its probe is reported but never recorded as the image's state.
export function reportGoogleSession(vm: string, options: {record: boolean}): void {
  try {
    copyGuestTools(vm)
    const previous = options.record ? readHostImageState().google?.state : undefined
    const state = probeGoogle(vm)
    if (options.record) recordGoogleState(state)
    const message = expiryMessage(state, {vm, previous})
    if (message) warn(message.text)
  } catch (error) {
    warn(`could not check the Google session in ${vm}: ${causeOf(error)}`)
  }
}
