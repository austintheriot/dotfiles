import {z} from 'zod'

export type Proc = {pid: number; ppid: number; started: string; command: string}
export type Owner = {pid: number; started: string}
export type Lease = {vm: string; owner: Owner}
export type Vm = {name: string; running: boolean}
export type Reclaim = {vm: string; reason: string}
export type Claim = {ok: true} | {ok: false; reason: string}

const QA_VM = /^pr-qa-(\d+)$/
const PS_LINE = /^\s*(\d+)\s+(\d+)\s+(\w{3} \w{3}\s+\d+ \d\d:\d\d:\d\d \d{4})\s+(.+)$/

// Expects `ps -A -o pid=,ppid=,lstart=,comm=` run with LC_ALL=C.
export function parseProcessTable(output: string): Proc[] {
  return output.split('\n').flatMap(line => {
    const match = PS_LINE.exec(line)
    if (!match) return []
    const [, pid, ppid, started, command] = match
    return [{pid: Number(pid), ppid: Number(ppid), started: (started ?? '').replace(/\s+/g, ' '), command: (command ?? '').trim()}]
  })
}

const TartList = z.array(z.object({Name: z.string(), State: z.string()}))
const LeaseFile = z.object({vm: z.string().regex(QA_VM), owner: z.object({pid: z.number().int().positive(), started: z.string()})})

const parseJson = (text: string): unknown => {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

// Expects `tart list --format json`.
export function parseTartList(json: string): Vm[] {
  const parsed = TartList.safeParse(parseJson(json))
  if (!parsed.success) throw new Error(`tart list returned an unexpected shape: ${parsed.error.issues[0]?.message ?? 'not JSON'}`)
  return parsed.data.flatMap(entry => (QA_VM.test(entry.Name) ? [{name: entry.Name, running: entry.State === 'running'}] : []))
}

export function parseLease(text: string): Lease | undefined {
  const parsed = LeaseFile.safeParse(parseJson(text))
  return parsed.success ? parsed.data : undefined
}

const sameProcess = (table: Proc[], owner: Owner) => table.some(entry => entry.pid === owner.pid && entry.started === owner.started)
const asOwner = (entry: Proc): Owner => ({pid: entry.pid, started: entry.started})
const isClaude = (entry: Proc) => entry.command.split('/').at(-1) === 'claude'

// The owner is the long-lived process the VM serves: the process the caller names (a scheduler does),
// or else the nearest claude session above the caller.
export function findOwner(table: Proc[], callerPid: number, namedPid?: number): Owner | undefined {
  const byPid = new Map(table.map(entry => [entry.pid, entry]))
  if (namedPid !== undefined) {
    const named = byPid.get(namedPid)
    return named && asOwner(named)
  }
  const ancestors = (pid: number, seen: Set<number>): Proc | undefined => {
    const entry = byPid.get(pid)
    if (!entry || seen.has(pid)) return undefined
    return isClaude(entry) ? entry : ancestors(entry.ppid, new Set([...seen, pid]))
  }
  const claude = ancestors(callerPid, new Set())
  return claude && asOwner(claude)
}

// A VM is reclaimed only when nothing can still be using it. An unknown owner keeps a running VM.
export function planReclaim(vms: Vm[], leases: Lease[], table: Proc[]): Reclaim[] {
  return vms.flatMap(vm => {
    const held = leases.find(entry => entry.vm === vm.name)
    if (held) return sameProcess(table, held.owner) ? [] : [{vm: vm.name, reason: `its owner, pid ${held.owner.pid}, exited`}]
    return vm.running ? [] : [{vm: vm.name, reason: 'it is stopped and has no lease'}]
  })
}

export function claimVm(name: string, vms: Vm[], leases: Lease[], table: Proc[], self: Owner | undefined): Claim {
  const vm = vms.find(entry => entry.name === name)
  if (!vm) return {ok: true}
  const held = leases.find(entry => entry.vm === name)
  if (held) {
    const mine = self !== undefined && held.owner.pid === self.pid && held.owner.started === self.started
    return mine || !sameProcess(table, held.owner) ? {ok: true} : {ok: false, reason: `${name} belongs to pid ${held.owner.pid}, which is still running`}
  }
  if (!vm.running) return {ok: true}
  const pr = QA_VM.exec(name)?.[1] ?? name
  return {ok: false, reason: `${name} is running and has no lease, so its owner is unknown. If nothing uses it, run vm down ${pr}, then vm up`}
}

export type VmRow = {vm: string; state: 'running' | 'stopped'; owner: string}

export function describeVms(vms: Vm[], leases: Lease[], table: Proc[]): VmRow[] {
  return vms.map(vm => {
    const held = leases.find(entry => entry.vm === vm.name)
    const running = held && table.find(entry => entry.pid === held.owner.pid && entry.started === held.owner.started)
    const owner = !held ? 'unknown' : running ? `pid ${held.owner.pid} (${running.command.split('/').at(-1)})` : `pid ${held.owner.pid} (exited)`
    return {vm: vm.name, state: vm.running ? 'running' : 'stopped', owner}
  })
}

export type RestartPlan = {kind: 'missing'} | {kind: 'refused'; reason: string} | {kind: 'restart'}

export function planRestart(name: string, vms: Vm[], leases: Lease[], table: Proc[], self: Owner | undefined): RestartPlan {
  if (!vms.some(entry => entry.name === name)) return {kind: 'missing'}
  const claim = claimVm(name, vms, leases, table, self)
  return claim.ok ? {kind: 'restart'} : {kind: 'refused', reason: claim.reason}
}

// A boot check against a VM that never stopped finds the old instance and reports a reboot that did not happen.
export function confirmStopped(name: string, vms: Vm[], stop: {code: number; stderr: string}): Claim {
  const vm = vms.find(entry => entry.name === name)
  if (!vm) return {ok: false, reason: `${name} disappeared during tart stop`}
  if (!vm.running) return {ok: true}
  const detail = stop.stderr.trim()
  return {ok: false, reason: `${name} is still running after tart stop${detail === '' ? '' : `: ${detail}`}`}
}
