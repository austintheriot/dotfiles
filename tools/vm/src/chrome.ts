import {CHROME_PORT, VM_SCREEN} from './config'
import {chromePolicy, plistXml, WRITE_CHROME_POLICY_COMMAND} from './image/grants'
import {sleep, succeeds, tartExec, warn} from './proc'

// Media access comes from the Chrome policy that startChrome writes at every launch, because macOS
// clears Managed Preferences at boot. --use-fake-ui-for-media-stream would show an unsupported-flag
// infobar in every recording. The startup policy opens the one blank tab.
export const CHROME_ARGS: readonly string[] = [
  '--user-data-dir=$HOME/qa-chrome-profile',
  `--remote-debugging-port=${CHROME_PORT}`,
  '--no-first-run',
  '--no-default-browser-check',
  '--hide-crash-restore-bubble',
  '--window-position=0,0',
  `--window-size=${VM_SCREEN.width},${VM_SCREEN.height}`
]

// A normally launched Chrome on the image's signed-in profile. Playwright attaches over CDP,
// so Google sees no automation flags and the profile's cookies stay readable.
export function startChrome(vm: string, mediaOrigins: readonly string[]): boolean {
  const policy = tartExec(vm, WRITE_CHROME_POLICY_COMMAND, {stdin: plistXml(chromePolicy(mediaOrigins))})
  if (policy.code !== 0) warn(`could not write the Chrome policy in ${vm}, so Chrome starts without the media policy and a permission bubble can cover recordings`)
  tartExec(vm, `(open -na '/Applications/Google Chrome.app' --args ${CHROME_ARGS.join(' ')} &>/dev/null &)`)
  for (let attempt = 0; attempt < 30; attempt++) {
    if (succeeds(['tart', 'exec', vm, 'curl', '-sf', `http://127.0.0.1:${CHROME_PORT}/json/version`])) return true
    sleep(1000)
  }
  console.error(`Chrome in ${vm} never opened port ${CHROME_PORT}`)
  return false
}

function chromeGone(vm: string): boolean {
  return tartExec(vm, 'pgrep -x "Google Chrome" >/dev/null').code !== 0
}

function waitForChromeToExit(vm: string, seconds: number): boolean {
  for (let attempt = 0; attempt < seconds; attempt++) {
    if (chromeGone(vm)) return true
    sleep(1000)
  }
  return false
}

// Chrome started with open -na and its own --user-data-dir can ignore the AppleScript quit,
// and Chrome on macOS treats SIGTERM as a clean shutdown.
export function quitChrome(vm: string): boolean {
  tartExec(vm, 'osascript -e \'tell application "Google Chrome" to quit\' 2>/dev/null; true')
  if (waitForChromeToExit(vm, 5)) return true
  tartExec(vm, 'pkill -TERM -x "Google Chrome"; true')
  return waitForChromeToExit(vm, 10)
}
