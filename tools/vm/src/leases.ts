import {existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {join} from 'node:path'
import {LEASE_DIR, EXIT} from './config'
import {claimVm, findOwner, parseLease, parseProcessTable, parseTartList, planReclaim, type Lease, type Owner, type Proc, type Vm} from './lease'
import {output, run, VmError, warn} from './proc'

const leasePath = (vm: string) => join(LEASE_DIR, `${vm}.json`)

export const tartVms = (): Vm[] => parseTartList(output(['tart', 'list', '--format', 'json']))
export const processTable = (): Proc[] => parseProcessTable(output(['/bin/ps', '-A', '-o', 'pid=,ppid=,lstart=,comm='], {env: {LC_ALL: 'C'}}))

export function readLeases(): Lease[] {
  if (!existsSync(LEASE_DIR)) return []
  return readdirSync(LEASE_DIR).flatMap(file => parseLease(readFileSync(join(LEASE_DIR, file), 'utf8')) ?? [])
}

export const releaseLease = (vm: string) => rmSync(leasePath(vm), {force: true})

export type Ownership = {vms: Vm[]; table: Proc[]; leases: Lease[]; owner: Owner | undefined}

export function readOwnership(env: Record<string, string | undefined>): Ownership {
  const vms = tartVms()
  const table = processTable()
  const named = env['QA_VM_OWNER_PID']
  return {vms, table, leases: readLeases(), owner: findOwner(table, process.pid, named ? Number(named) : undefined)}
}

// Refuses a VM another live session owns, reclaims abandoned VMs, and takes the lease before the clone,
// so a concurrent up never mistakes the new clone for an abandoned one.
export function prepareLease(vm: string, env: Record<string, string | undefined>): void {
  const {vms, table, leases, owner} = readOwnership(env)

  const claim = claimVm(vm, vms, leases, table, owner)
  if (!claim.ok) throw new VmError(`refusing ${vm}: ${claim.reason}`, EXIT.inUse)

  for (const {vm: abandoned, reason} of planReclaim(vms, leases, table)) {
    run(['tart', 'stop', abandoned])
    const deleted = run(['tart', 'delete', abandoned]).code === 0
    if (deleted) releaseLease(abandoned)
    console.error(deleted ? `reclaimed ${abandoned}: ${reason}` : `could not reclaim ${abandoned} (${reason})`)
  }

  takeLease(vm, owner)
}

export function takeLease(vm: string, owner: Owner | undefined): void {
  mkdirSync(LEASE_DIR, {recursive: true})
  if (owner === undefined) {
    releaseLease(vm)
    warn(`no owner found for ${vm}, so it is never reclaimed automatically. Run vm down when done`)
  } else {
    writeFileSync(leasePath(vm), `${JSON.stringify({vm, owner} satisfies Lease)}\n`)
  }
}
