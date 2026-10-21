import {describe, expect, test} from 'bun:test'
import {claimVm, confirmStopped, describeVms, findOwner, parseLease, parseProcessTable, parseTartList, planReclaim, planRestart, type Lease, type Proc} from './lease'

const STARTED = 'Wed Oct 14 10:00:21 2026'
const proc = (pid: number, ppid: number, command: string, started = STARTED): Proc => ({pid, ppid, started, command})
const lease = (vm: string, pid: number, started = STARTED): Lease => ({vm, owner: {pid, started}})

describe('parseProcessTable', () => {
  test('reads pid, parent, start time, and command from ps output', () => {
    const output = [' 96832 10844 Wed Oct 14 10:00:21 2026     /bin/zsh', '10844  8218 Fri Oct  9 10:08:28 2026     claude', ''].join('\n')
    expect(parseProcessTable(output)).toEqual([
      {pid: 96832, ppid: 10844, started: 'Wed Oct 14 10:00:21 2026', command: '/bin/zsh'},
      {pid: 10844, ppid: 8218, started: 'Fri Oct 9 10:08:28 2026', command: 'claude'}
    ])
  })

  test('skips lines it cannot read', () => {
    expect(parseProcessTable('garbage\n')).toEqual([])
  })
})

describe('parseTartList', () => {
  test('keeps only per-PR QA VMs', () => {
    const json = JSON.stringify([
      {Name: 'pr-qa-1165', State: 'stopped'},
      {Name: 'pr-qa-64420', State: 'running'},
      {Name: 'pr-qa-base', State: 'stopped'},
      {Name: 'ghcr.io/cirruslabs/macos-tahoe-base:latest', State: 'stopped'}
    ])
    expect(parseTartList(json)).toEqual([
      {name: 'pr-qa-1165', running: false},
      {name: 'pr-qa-64420', running: true}
    ])
  })

  test('refuses output that is not a VM list', () => {
    expect(() => parseTartList('{}')).toThrow('tart list')
    expect(() => parseTartList('[{"Name": 3}]')).toThrow('tart list')
  })
})

describe('parseLease', () => {
  test('reads a lease file', () => {
    expect(parseLease('{"vm":"pr-qa-1","owner":{"pid":100,"started":"Wed Oct 14 10:00:21 2026"}}')).toEqual(lease('pr-qa-1', 100))
  })

  test('ignores a damaged lease, so its VM counts as unowned', () => {
    expect(parseLease('{"vm":"pr-qa-1"}')).toBeUndefined()
    expect(parseLease('not json')).toBeUndefined()
  })
})

describe('describeVms', () => {
  test('shows each VM with its owner and whether the owner still runs', () => {
    const table = [proc(100, 1, 'claude')]
    const vms = [{name: 'pr-qa-1', running: true}, {name: 'pr-qa-2', running: false}, {name: 'pr-qa-3', running: true}]
    expect(describeVms(vms, [lease('pr-qa-1', 100), lease('pr-qa-2', 300)], table)).toEqual([
      {vm: 'pr-qa-1', state: 'running', owner: 'pid 100 (claude)'},
      {vm: 'pr-qa-2', state: 'stopped', owner: 'pid 300 (exited)'},
      {vm: 'pr-qa-3', state: 'running', owner: 'unknown'}
    ])
  })
})

describe('findOwner', () => {
  const table = [proc(300, 200, '/bin/zsh'), proc(200, 100, '/bin/zsh'), proc(100, 50, 'claude'), proc(50, 1, 'tmux'), proc(77, 1, '/bin/zsh')]

  test('is the nearest claude session above the caller', () => {
    expect(findOwner(table, 300)).toEqual({pid: 100, started: STARTED})
  })

  test('is the process the caller names, such as a scheduler', () => {
    expect(findOwner(table, 300, 77)).toEqual({pid: 77, started: STARTED})
  })

  test('is unknown when no claude session is above the caller', () => {
    expect(findOwner([proc(300, 1, '/bin/zsh')], 300)).toBeUndefined()
  })

  test('is unknown when the named process is not running', () => {
    expect(findOwner(table, 300, 999)).toBeUndefined()
  })

  test('stops at a parent loop', () => {
    expect(findOwner([proc(1, 2, 'zsh'), proc(2, 1, 'zsh')], 1)).toBeUndefined()
  })
})

describe('planReclaim', () => {
  const table = [proc(100, 1, 'claude'), proc(200, 1, 'claude', 'Thu Oct 15 08:00:00 2026')]

  test('keeps a VM whose owner is still running', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 100)], table)).toEqual([])
  })

  test('reclaims a VM whose owner exited', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 300)], table)).toEqual([
      {vm: 'pr-qa-1', reason: 'its owner, pid 300, exited'}
    ])
  })

  test('reclaims a VM whose owner pid now belongs to another process', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 200)], table)).toEqual([
      {vm: 'pr-qa-1', reason: 'its owner, pid 200, exited'}
    ])
  })

  test('keeps a running VM with no lease, because its owner is unknown', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: true}], [], table)).toEqual([])
  })

  test('reclaims a stopped VM with no lease, because nothing can use it', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: false}], [], table)).toEqual([{vm: 'pr-qa-1', reason: 'it is stopped and has no lease'}])
  })

  test('keeps a stopped VM whose owner is still running, because the owner may be cloning it', () => {
    expect(planReclaim([{name: 'pr-qa-1', running: false}], [lease('pr-qa-1', 100)], table)).toEqual([])
  })
})

describe('claimVm', () => {
  const table = [proc(100, 1, 'claude'), proc(200, 1, 'claude')]
  const self = {pid: 100, started: STARTED}

  test('allows a VM that does not exist', () => {
    expect(claimVm('pr-qa-1', [], [], table, self)).toEqual({ok: true})
  })

  test('allows the same owner to recreate its own VM', () => {
    expect(claimVm('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 100)], table, self)).toEqual({ok: true})
  })

  test('refuses a VM another running session owns', () => {
    expect(claimVm('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 200)], table, self)).toEqual({
      ok: false,
      reason: 'pr-qa-1 belongs to pid 200, which is still running'
    })
  })

  test('refuses a running VM with no lease, because its owner is unknown', () => {
    expect(claimVm('pr-qa-1', [{name: 'pr-qa-1', running: true}], [], table, self)).toEqual({
      ok: false,
      reason: 'pr-qa-1 is running and has no lease, so its owner is unknown. If nothing uses it, run vm down 1, then vm up'
    })
  })

  test('allows a VM whose owner exited', () => {
    expect(claimVm('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 300)], table, self)).toEqual({ok: true})
  })

  test('refuses another session even when the caller has no known owner', () => {
    expect(claimVm('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 200)], table, undefined).ok).toBe(false)
  })
})

describe('planRestart', () => {
  const table = [proc(100, 1, 'claude'), proc(200, 1, 'claude')]
  const self = {pid: 100, started: STARTED}

  test('names a VM that does not exist', () => {
    expect(planRestart('pr-qa-1', [], [], table, self)).toEqual({kind: 'missing'})
  })

  test('restarts the caller\'s own running VM', () => {
    expect(planRestart('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 100)], table, self)).toEqual({kind: 'restart'})
  })

  test('restarts a stopped VM with no lease', () => {
    expect(planRestart('pr-qa-1', [{name: 'pr-qa-1', running: false}], [], table, self)).toEqual({kind: 'restart'})
  })

  test('refuses a VM another live session owns, the same rule up uses', () => {
    expect(planRestart('pr-qa-1', [{name: 'pr-qa-1', running: true}], [lease('pr-qa-1', 200)], table, self)).toEqual({
      kind: 'refused',
      reason: 'pr-qa-1 belongs to pid 200, which is still running'
    })
  })

  test('refuses a running VM with no lease, because its owner is unknown', () => {
    expect(planRestart('pr-qa-1', [{name: 'pr-qa-1', running: true}], [], table, self).kind).toBe('refused')
  })
})

describe('confirmStopped', () => {
  test('accepts a VM that tart list shows stopped, whatever tart stop returned', () => {
    expect(confirmStopped('pr-qa-1', [{name: 'pr-qa-1', running: false}], {code: 0, stderr: ''})).toEqual({ok: true})
    expect(confirmStopped('pr-qa-1', [{name: 'pr-qa-1', running: false}], {code: 1, stderr: 'VM is not running'})).toEqual({ok: true})
  })

  test('refuses a VM that is still running after tart stop, and names the stop error', () => {
    expect(confirmStopped('pr-qa-1', [{name: 'pr-qa-1', running: true}], {code: 1, stderr: 'timed out\n'})).toEqual({
      ok: false,
      reason: 'pr-qa-1 is still running after tart stop: timed out'
    })
  })

  test('refuses a VM that is still running even when tart stop exited 0', () => {
    expect(confirmStopped('pr-qa-1', [{name: 'pr-qa-1', running: true}], {code: 0, stderr: ''})).toEqual({
      ok: false,
      reason: 'pr-qa-1 is still running after tart stop'
    })
  })

  test('refuses a VM that tart list no longer shows', () => {
    expect(confirmStopped('pr-qa-1', [], {code: 0, stderr: ''})).toEqual({ok: false, reason: 'pr-qa-1 disappeared during tart stop'})
  })
})
