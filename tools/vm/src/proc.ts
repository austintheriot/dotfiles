import {closeSync, fstatSync, openSync} from 'node:fs'
import type {Writable} from 'node:stream'

export class VmError extends Error {
  public constructor(
    message: string,
    public readonly exitCode: number
  ) {
    super(message)
  }
}

type Io = 'inherit' | 'pipe' | 'ignore'
export type RunOptions = {stdin?: string | 'inherit'; stdout?: Io | {file: string}; stderr?: Io; env?: Record<string, string>}
export type RunResult = {code: number; stdout: string; stderr: string}

export const isTransientAgentError = (stderr: string) => stderr.includes('GRPCConnectionPoolError')

export function withTransientRetry(runner: () => RunResult, pause: () => void): RunResult {
  const first = runner()
  if (first.code === 0 || !isTransientAgentError(first.stderr)) return first
  pause()
  return runner()
}

export type ExecAttempt = {code: number; stderr: string; wroteStdout: boolean; stdinPiped: boolean; attemptNumber: number}

export const shouldRetryExec = (attempt: ExecAttempt) =>
  attempt.attemptNumber === 1 && attempt.code !== 0 && !attempt.wroteStdout && !attempt.stdinPiped && isTransientAgentError(attempt.stderr)

export type StdinKind = 'fifo' | 'file' | 'socket' | 'character-device' | 'other' | 'closed'
type StdinStats = {isFIFO: () => boolean; isFile: () => boolean; isSocket: () => boolean; isCharacterDevice: () => boolean}

export function stdinKind(stats: StdinStats | undefined): StdinKind {
  if (stats === undefined) return 'closed'
  if (stats.isFIFO()) return 'fifo'
  if (stats.isFile()) return 'file'
  if (stats.isSocket()) return 'socket'
  if (stats.isCharacterDevice()) return 'character-device'
  return 'other'
}

// A terminal and /dev/null are both character devices. Claude Code's Bash tool gives a command /dev/null.
export const stdinCarriesData = (kind: StdinKind) => kind === 'fifo' || kind === 'file' || kind === 'socket'

function statStdin(): StdinStats | undefined {
  try {
    return fstatSync(0)
  } catch {
    return undefined
  }
}

export const hostStdinCarriesData = () => stdinCarriesData(stdinKind(statStdin()))

export type ExecResult = {code: number; stdoutClosed: boolean}

export const shouldDiagnoseNetwork = (result: ExecResult) => result.code !== 0 && !result.stdoutClosed

export type Sinks = {stdout: Writable; stderr: Writable}

const EXEC_STDOUT_CLOSED = 141
const STDERR_TAIL_BYTES = 64 * 1024

class SinkClosed extends Error {}

function writeChunk(sink: Writable, chunk: Uint8Array): Promise<void> {
  return new Promise((resolve, reject) => {
    const onError = () => reject(new SinkClosed())
    sink.once('error', onError)
    const flushed = sink.write(chunk, error => {
      if (error) onError()
    })
    if (flushed) {
      sink.off('error', onError)
      resolve()
      return
    }
    sink.once('drain', () => {
      sink.off('error', onError)
      resolve()
    })
  })
}

function flush(sink: Writable): Promise<void> {
  return new Promise((resolve, reject) => {
    sink.once('error', () => reject(new SinkClosed()))
    sink.write('', error => (error ? reject(new SinkClosed()) : resolve()))
  })
}

async function forward(stream: ReadableStream<Uint8Array>, sink: Writable, onChunk: (text: string) => void): Promise<void> {
  const decoder = new TextDecoder()
  for await (const chunk of stream) {
    await writeChunk(sink, chunk)
    onChunk(decoder.decode(chunk, {stream: true}))
  }
  await flush(sink)
}

const processSinks = (): Sinks => ({stdout: process.stdout, stderr: process.stderr})

export async function runStreaming(
  command: string[],
  stdinPiped: boolean,
  sinks: Sinks = processSinks()
): Promise<{code: number; stderr: string; wroteStdout: boolean; stdoutClosed: boolean}> {
  const child = Bun.spawn(command, {stdin: stdinPiped ? 'inherit' : 'ignore', stdout: 'pipe', stderr: 'pipe'})
  let stderr = ''
  let wroteStdout = false
  let stdoutClosed = false
  const swallow = () => {}
  for (const sink of [sinks.stdout, sinks.stderr]) sink.on('error', swallow)
  const forwarding = Promise.all([
    forward(child.stdout, sinks.stdout, () => (wroteStdout = true)).catch(error => {
      if (!(error instanceof SinkClosed)) throw error
      stdoutClosed = true
      child.kill()
    }),
    forward(child.stderr, sinks.stderr, text => (stderr = (stderr + text).slice(-STDERR_TAIL_BYTES))).catch(error => {
      if (!(error instanceof SinkClosed)) throw error
    })
  ])
  await forwarding
  for (const sink of [sinks.stdout, sinks.stderr]) sink.off('error', swallow)
  return {code: await child.exited, stderr, wroteStdout, stdoutClosed}
}

const execResult = (attempt: {code: number; stdoutClosed: boolean}): ExecResult =>
  attempt.stdoutClosed ? {code: EXEC_STDOUT_CLOSED, stdoutClosed: true} : {code: attempt.code, stdoutClosed: false}

export async function runExecWithRetry(command: string[], stdinPiped: boolean, sinks: Sinks = processSinks()): Promise<ExecResult> {
  const first = await runStreaming(command, stdinPiped, sinks)
  if (first.stdoutClosed || !shouldRetryExec({...first, stdinPiped, attemptNumber: 1})) return execResult(first)
  sinks.stderr.write('\x1b[2mvm: guest agent not ready, retrying once\x1b[0m\n')
  await Bun.sleep(1000)
  return execResult(await runStreaming(command, stdinPiped, sinks))
}

const isTartExec = (command: string[]) => command[0] === 'tart' && command[1] === 'exec'

export function run(command: string[], options: RunOptions = {}): RunResult {
  const retryable = isTartExec(command) && options.stdin !== 'inherit' && (options.stderr ?? 'pipe') === 'pipe'
  return retryable ? withTransientRetry(() => runOnce(command, options), () => sleep(1000)) : runOnce(command, options)
}

function runOnce(command: string[], options: RunOptions): RunResult {
  const fd = typeof options.stdout === 'object' ? openSync(options.stdout.file, 'w') : undefined
  try {
    const result = Bun.spawnSync(command, {
      stdin: options.stdin === undefined ? 'ignore' : options.stdin === 'inherit' ? 'inherit' : Buffer.from(options.stdin),
      stdout: fd ?? (typeof options.stdout === 'string' ? options.stdout : 'pipe'),
      stderr: options.stderr ?? 'pipe',
      env: {...process.env, ...options.env}
    })
    return {code: result.exitCode ?? 1, stdout: result.stdout?.toString() ?? '', stderr: result.stderr?.toString() ?? ''}
  } finally {
    if (fd !== undefined) closeSync(fd)
  }
}

export function succeeds(command: string[], options: RunOptions = {}): boolean {
  return run(command, {stdout: 'ignore', stderr: 'ignore', ...options}).code === 0
}

export function output(command: string[], options: RunOptions = {}): string {
  const result = run(command, options)
  if (result.code !== 0) throw new VmError(`${command.slice(0, 3).join(' ')} failed: ${result.stderr.trim()}`, 1)
  return result.stdout
}

// Runs a zsh script whose positional parameters carry the values, so no value is ever quoted into the script.
export function zsh(script: string, args: string[], options: RunOptions = {}): RunResult {
  return run(['/bin/zsh', '-c', script, 'zsh', ...args], options)
}

export const tartExec = (vm: string, script: string, options: RunOptions = {}) =>
  run(['tart', 'exec', ...(options.stdin === undefined ? [] : ['-i']), vm, 'zsh', '-lc', script], options)

export const warn = (message: string) => console.error(`WARNING: ${message}`)

export const sleep = (ms: number) => Bun.sleepSync(ms)
