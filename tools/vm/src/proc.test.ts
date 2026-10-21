import {describe, expect, test} from 'bun:test'
import {mkdtempSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {Writable} from 'node:stream'
import {isTransientAgentError, runExecWithRetry, runStreaming, type RunResult, shouldDiagnoseNetwork, shouldRetryExec, stdinCarriesData, stdinKind, withTransientRetry} from './proc'

const AGENT_ERROR = 'Error: GRPC.GRPCConnectionPoolError(wrapped: ...). Is the Tart Guest Agent running?'

function sequence(results: RunResult[]) {
  const calls: number[] = []
  const runner = () => {
    calls.push(calls.length)
    const result = results[Math.min(calls.length - 1, results.length - 1)]
    if (result === undefined) throw new Error('sequence needs at least one result')
    return result
  }
  return {runner, calls}
}

describe('isTransientAgentError', () => {
  test('matches the guest agent connection pool error', () => {
    expect(isTransientAgentError(AGENT_ERROR)).toBe(true)
  })

  test('rejects other stderr text', () => {
    expect(isTransientAgentError('zsh: command not found: foo')).toBe(false)
    expect(isTransientAgentError('')).toBe(false)
  })
})

describe('withTransientRetry', () => {
  test('returns a success without a retry', () => {
    const {runner, calls} = sequence([{code: 0, stdout: 'ok', stderr: ''}])
    const pauses: number[] = []
    expect(withTransientRetry(runner, () => pauses.push(1)).stdout).toBe('ok')
    expect(calls).toHaveLength(1)
    expect(pauses).toHaveLength(0)
  })

  test('retries once after the transient error and returns the second result', () => {
    const {runner, calls} = sequence([
      {code: 1, stdout: '', stderr: AGENT_ERROR},
      {code: 0, stdout: 'ok', stderr: ''}
    ])
    const pauses: number[] = []
    expect(withTransientRetry(runner, () => pauses.push(1))).toEqual({code: 0, stdout: 'ok', stderr: ''})
    expect(calls).toHaveLength(2)
    expect(pauses).toHaveLength(1)
  })

  test('retries only once when the error repeats', () => {
    const failed = {code: 1, stdout: '', stderr: AGENT_ERROR}
    const {runner, calls} = sequence([failed])
    expect(withTransientRetry(runner, () => {})).toEqual(failed)
    expect(calls).toHaveLength(2)
  })

  test('does not retry a different failure', () => {
    const {runner, calls} = sequence([{code: 2, stdout: '', stderr: 'boom'}])
    expect(withTransientRetry(runner, () => {}).code).toBe(2)
    expect(calls).toHaveLength(1)
  })
})

describe('shouldRetryExec', () => {
  const base = {code: 1, stderr: AGENT_ERROR, wroteStdout: false, stdinPiped: false, attemptNumber: 1}

  test('retries a first attempt that failed with the transient error before any output', () => {
    expect(shouldRetryExec(base)).toBe(true)
  })

  test.each([
    ['exit code is zero', {code: 0}],
    ['stderr is a different error', {stderr: 'boom'}],
    ['stdout already received output', {wroteStdout: true}],
    ['stdin is piped', {stdinPiped: true}],
    ['it is already the second attempt', {attemptNumber: 2}]
  ])('does not retry when %s', (_name, override) => {
    expect(shouldRetryExec({...base, ...override})).toBe(false)
  })
})

describe('stdinKind', () => {
  const stats = (kind: 'fifo' | 'file' | 'socket' | 'character-device' | 'directory') => ({
    isFIFO: () => kind === 'fifo',
    isFile: () => kind === 'file',
    isSocket: () => kind === 'socket',
    isCharacterDevice: () => kind === 'character-device'
  })

  test.each(['fifo', 'file', 'socket', 'character-device'] as const)('reads a %s', kind => {
    expect(stdinKind(stats(kind))).toBe(kind)
  })

  test('calls anything else other', () => {
    expect(stdinKind(stats('directory'))).toBe('other')
  })

  test('calls a closed stdin closed', () => {
    expect(stdinKind(undefined)).toBe('closed')
  })
})

describe('stdinCarriesData', () => {
  test.each(['fifo', 'file', 'socket'] as const)('is true for a %s, which a caller pipes or redirects', kind => {
    expect(stdinCarriesData(kind)).toBe(true)
  })

  test.each(['character-device', 'other', 'closed'] as const)('is false for %s, which covers a terminal and /dev/null under an agent', kind => {
    expect(stdinCarriesData(kind)).toBe(false)
  })
})

describe('shouldDiagnoseNetwork', () => {
  test('diagnoses a failed command', () => {
    expect(shouldDiagnoseNetwork({code: 1, stdoutClosed: false})).toBe(true)
  })

  test('skips a success', () => {
    expect(shouldDiagnoseNetwork({code: 0, stdoutClosed: false})).toBe(false)
  })

  test('skips a run whose host stdout reader closed, whatever its code', () => {
    expect(shouldDiagnoseNetwork({code: 141, stdoutClosed: true})).toBe(false)
    expect(shouldDiagnoseNetwork({code: 1, stdoutClosed: true})).toBe(false)
  })

  test('diagnoses a guest command that itself exits 141', () => {
    expect(shouldDiagnoseNetwork({code: 141, stdoutClosed: false})).toBe(true)
  })
})

function collectingSink(delayMs = 0) {
  const chunks: Buffer[] = []
  const sink = new Writable({
    highWaterMark: 16 * 1024,
    write(chunk: Buffer, _encoding, done) {
      chunks.push(chunk)
      if (delayMs === 0) done()
      else setTimeout(done, delayMs)
    }
  })
  return {sink, text: () => Buffer.concat(chunks).toString(), bytes: () => chunks.reduce((total, chunk) => total + chunk.length, 0)}
}

function brokenPipeSink() {
  return new Writable({
    write(_chunk, _encoding, done) {
      done(Object.assign(new Error('EPIPE: broken pipe, write'), {code: 'EPIPE'}))
    }
  })
}

describe('runStreaming', () => {
  test('delivers every byte to a slow sink before it resolves', async () => {
    const out = collectingSink(1)
    const result = await runStreaming(['/bin/sh', '-c', 'head -c 5000000 /dev/zero'], false, {stdout: out.sink, stderr: collectingSink().sink})
    expect(result.code).toBe(0)
    expect(out.bytes()).toBe(5000000)
  })

  test('stops and kills the child when the stdout reader is gone', async () => {
    const result = await runStreaming(['/bin/sh', '-c', 'yes'], false, {stdout: brokenPipeSink(), stderr: collectingSink().sink})
    expect(result.stdoutClosed).toBe(true)
  })

  test('keeps only a bounded tail of stderr', async () => {
    const err = collectingSink()
    const result = await runStreaming(['/bin/sh', '-c', 'head -c 300000 /dev/zero | tr "\\0" x >&2; echo END >&2'], false, {stdout: collectingSink().sink, stderr: err.sink})
    expect(result.stderr.length).toBeLessThanOrEqual(65536)
    expect(result.stderr.endsWith('END\n')).toBe(true)
    expect(err.bytes()).toBe(300004)
  })
})

describe('runExecWithRetry', () => {
  test('retries once after the transient error and says so on stderr', async () => {
    const marker = join(mkdtempSync(join(tmpdir(), 'vm-retry-')), 'seen')
    const script = `if [ -f "${marker}" ]; then echo ok; else touch "${marker}"; echo GRPCConnectionPoolError >&2; exit 1; fi`
    const out = collectingSink()
    const err = collectingSink()
    const result = await runExecWithRetry(['/bin/sh', '-c', script], false, {stdout: out.sink, stderr: err.sink})
    expect(result).toEqual({code: 0, stdoutClosed: false})
    expect(out.text()).toBe('ok\n')
    expect(err.text()).toContain('guest agent not ready, retrying once')
  })

  test('returns 141 and says the stdout reader closed, without a retry', async () => {
    const err = collectingSink()
    const result = await runExecWithRetry(['/bin/sh', '-c', 'yes'], false, {stdout: brokenPipeSink(), stderr: err.sink})
    expect(result).toEqual({code: 141, stdoutClosed: true})
    expect(err.text()).not.toContain('retrying')
  })

  test('returns a guest exit code of 141 as an ordinary code', async () => {
    const result = await runExecWithRetry(['/bin/sh', '-c', 'exit 141'], false, {stdout: collectingSink().sink, stderr: collectingSink().sink})
    expect(result).toEqual({code: 141, stdoutClosed: false})
  })
})
