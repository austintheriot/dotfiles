import {vmName} from './config'

export type NetworkProbe = {gateway: string | undefined; dnsResolves: boolean; hostAnswers: boolean}

export const PROBE_HOST = 'github.com'

// A TCP connect, not a ping: the Softnet gateway drops ICMP even when the network works.
export const PROBE_SCRIPT = `gw=$(route -n get default 2>/dev/null | awk '/gateway:/{print $2}')
echo "gateway=$gw"
if dscacheutil -q host -a name ${PROBE_HOST} 2>/dev/null | grep -q ip_address; then echo dns=ok; else echo dns=fail; fi
if nc -z -G 2 ${PROBE_HOST} 443 >/dev/null 2>&1; then echo tcp=ok; else echo tcp=fail; fi`

export function parseProbe(text: string): NetworkProbe | undefined {
  const fields = new Map(
    text
      .split('\n')
      .map(line => line.split('='))
      .filter((parts): parts is [string, string] => parts.length === 2)
  )
  const dns = fields.get('dns')
  const tcp = fields.get('tcp')
  if (!fields.has('gateway') || dns === undefined || tcp === undefined) return undefined
  return {gateway: fields.get('gateway') || undefined, dnsResolves: dns === 'ok', hostAnswers: tcp === 'ok'}
}

export function diagnoseNetwork(pr: number, probe: NetworkProbe): string | undefined {
  const fix = `Run vm restart ${pr}.`
  if (probe.gateway === undefined) return `${vmName(pr)} has no network: no default route. ${fix}`
  if (!probe.dnsResolves) return `${vmName(pr)} has no DNS: ${PROBE_HOST} does not resolve through gateway ${probe.gateway}. ${fix}`
  if (!probe.hostAnswers) return `${vmName(pr)} has no network: ${PROBE_HOST}:443 does not answer through gateway ${probe.gateway}. ${fix}`
  return undefined
}
