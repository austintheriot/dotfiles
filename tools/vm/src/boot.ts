import {spawn} from 'node:child_process'
import {existsSync, openSync, readFileSync} from 'node:fs'
import {EXIT} from './config'
import {sleep, succeeds, VmError} from './proc'

// Returns once the guest agent answers. tart run exits at once when the host already runs two macOS VMs.
function waitForBoot(vm: string, runner: number, log: string): void {
  for (let attempt = 0; attempt < 90; attempt++) {
    if (succeeds(['tart', 'exec', vm, 'true'])) return
    if (!isAlive(runner)) {
      const text = existsSync(log) ? readFileSync(log, 'utf8') : ''
      if (text.includes('exceeds the system limit')) throw new VmError(`${vm} did not start: the host is at its VM limit`, EXIT.tempFail)
      throw new VmError(`${vm} did not start: ${text.trim().split('\n').at(-1) ?? 'tart run exited'}`, EXIT.unavailable)
    }
    sleep(2000)
  }
  throw new VmError(`guest agent in ${vm} never answered`, EXIT.unavailable)
}

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

export function bootVm(vm: string, options: {window: boolean; softnet: boolean}): void {
  const log = `/tmp/${vm}-run.log`
  const fd = openSync(log, 'w')
  const flags = [...(options.window ? [] : ['--no-graphics']), '--no-audio', ...(options.softnet ? ['--net-softnet'] : [])]
  const runner = spawn('tart', ['run', ...flags, vm], {detached: true, stdio: ['ignore', fd, fd]})
  runner.unref()
  if (runner.pid === undefined) throw new VmError(`tart run did not start for ${vm}`, EXIT.unavailable)
  waitForBoot(vm, runner.pid, log)
}
