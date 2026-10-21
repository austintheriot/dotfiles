import {EXIT} from '../config'
import {tartExec, VmError, warn, type RunResult} from '../proc'
import {writeMarker} from './base-image'
import {chromePolicy, chromePolicyDigest, grantTable, plistXml, PROMPTS_OFF_SCRIPT, replaydScript, tccCheckSql, tccSql, WRITE_CHROME_POLICY_COMMAND, type TccDb} from './grants'

export const REPLAYD_SCRIPTED = false
export const LOCAL_NETWORK_SCRIPTED = false
export const LOCAL_NETWORK_CIDRS: readonly string[] = []

const TCC_DBS: Record<TccDb, string> = {
  system: '"/Library/Application Support/com.apple.TCC/TCC.db"',
  user: '"$HOME/Library/Application Support/com.apple.TCC/TCC.db"'
}

export function agentPath(vm: string): string {
  const resolved = tartExec(vm, 'realpath "$(command -v tart-guest-agent)"')
  const path = resolved.stdout.trim()
  if (resolved.code !== 0 || !path.startsWith('/')) throw new VmError(`could not find tart-guest-agent in ${vm}`, EXIT.unavailable)
  return path
}

function must(vm: string, what: string, result: RunResult): RunResult {
  if (result.code !== 0) throw new VmError(`${what} in ${vm} failed: ${result.stderr.trim()}`, EXIT.unavailable)
  return result
}

export function rewriteReplaydApproval(vm: string): void {
  if (!REPLAYD_SCRIPTED) return
  const written = tartExec(vm, replaydScript(agentPath(vm)))
  if (written.code !== 0) warn(`could not renew the screen-capture approval in ${vm}, so a recording can show a permission prompt`)
}

export function grantStep(vm: string, mediaOrigins: readonly string[]): void {
  if (!tartExec(vm, 'csrutil status').stdout.includes('disabled')) throw new VmError(`SIP is on in ${vm}; the TCC grants need it off`, EXIT.unavailable)
  const agent = agentPath(vm)
  const grants = grantTable(agent)
  const sql = tccSql(grants)
  const checkSql = tccCheckSql(grants)
  const counts: Record<TccDb, number> = {system: 0, user: 0}
  for (const db of ['system', 'user'] as const) {
    const sqlite = `sudo sqlite3 ${TCC_DBS[db]}`
    must(vm, `writing the ${db} TCC database`, tartExec(vm, sqlite, {stdin: sql[db]}))
    const expected = grants.filter(grant => grant.db === db).length
    const printed = must(vm, `checking the ${db} TCC database`, tartExec(vm, sqlite, {stdin: checkSql[db]})).stdout.trim()
    if (printed !== String(expected))
      throw new VmError(`the ${db} TCC database in ${vm} holds ${printed} of the ${expected} expected grants`, EXIT.unavailable)
    counts[db] = expected
  }
  tartExec(vm, 'sudo killall tccd 2>/dev/null; killall tccd 2>/dev/null; true')
  if (REPLAYD_SCRIPTED) must(vm, 'writing the screen-capture approval', tartExec(vm, replaydScript(agent)))
  must(
    vm,
    'writing the Chrome policy',
    tartExec(vm, WRITE_CHROME_POLICY_COMMAND, {stdin: plistXml(chromePolicy(mediaOrigins))})
  )
  must(vm, 'turning prompts off', tartExec(vm, PROMPTS_OFF_SCRIPT))
  if (LOCAL_NETWORK_SCRIPTED)
    must(
      vm,
      'allowing local network addresses',
      tartExec(vm, `sudo defaults write /Library/Preferences/com.apple.network.local-network AllowedEthernetLocalNetworkAddresses -array ${LOCAL_NETWORK_CIDRS.join(' ')}`)
    )
  console.log(`granted to ${agent}, Chrome, the desktop app, and Terminal (checked grants, system ${counts.system}, user ${counts.user})`)
  writeMarker(vm, 'grant', {agent, localNetwork: String(LOCAL_NETWORK_SCRIPTED), policy: chromePolicyDigest(mediaOrigins)})
}
