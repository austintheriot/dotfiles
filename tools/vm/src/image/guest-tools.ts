import {z} from 'zod'
import {CHROME_PORT, EXIT, GUEST_DIR, GUEST_TOOL_FILES, VM_BUN, VM_GUEST_TOOLS} from '../config'
import {run, VmError, zsh} from '../proc'
import {CdpCookie, classifyGoogleSession, toCookieParam, type GoogleSessionState} from './session'

export function copyGuestTools(vm: string): void {
  const copied = zsh(`set -o pipefail; tar -C "$1" -cf - ${GUEST_TOOL_FILES.join(' ')} | tart exec -i "$2" zsh -c "rm -rf $3 && mkdir -p $3 && tar -xf - -C $3"`, [GUEST_DIR, vm, VM_GUEST_TOOLS])
  if (copied.code !== 0) throw new VmError(`copying the guest tools into ${vm} failed: ${copied.stderr.trim()}`, EXIT.unavailable)
}

const tool = (vm: string, args: string[], stdin?: string) =>
  stdin === undefined
    ? run(['tart', 'exec', vm, VM_BUN, `${VM_GUEST_TOOLS}/cdp-session.ts`, ...args])
    : run(['tart', 'exec', '-i', vm, VM_BUN, `${VM_GUEST_TOOLS}/cdp-session.ts`, ...args], {stdin})

export function exportCookies(vm: string, port: number = CHROME_PORT): CdpCookie[] {
  const result = tool(vm, ['export', '--port', String(port)])
  if (result.code !== 0) throw new VmError(`could not read cookies from port ${port} in ${vm}: ${result.stderr.trim()}`, EXIT.unavailable)
  let json: unknown
  try {
    json = JSON.parse(result.stdout)
  } catch {
    throw new VmError(`the cookie export from ${vm} was not JSON`, EXIT.unavailable)
  }
  const cookies = z.array(CdpCookie).safeParse(json)
  if (!cookies.success) throw new VmError(`the cookie export from ${vm} had an unexpected shape`, EXIT.unavailable)
  return cookies.data
}

export function importCookies(vm: string, cookies: readonly CdpCookie[], target: {port: number; url: string | undefined}): void {
  const args = ['import', '--port', String(target.port), ...(target.url === undefined ? [] : ['--url', target.url])]
  const result = tool(vm, args, JSON.stringify(cookies.map(toCookieParam)))
  if (result.code !== 0) throw new VmError(`could not write cookies to port ${target.port} in ${vm}: ${result.stderr.trim()}`, result.code === EXIT.noInput ? EXIT.noInput : EXIT.unavailable)
}

export function probeGoogle(vm: string, port: number = CHROME_PORT): GoogleSessionState {
  const result = tool(vm, ['probe', '--port', String(port)])
  return result.code === 0 ? classifyGoogleSession(result.stdout.trim()) : 'unknown'
}

export function openTab(vm: string, url: string, port: number = CHROME_PORT): void {
  const result = tool(vm, ['open', '--port', String(port), url])
  if (result.code !== 0) throw new VmError(`could not open ${url} in ${vm}: ${result.stderr.trim()}`, EXIT.unavailable)
}
