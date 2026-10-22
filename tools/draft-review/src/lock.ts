import {dlopen, FFIType} from 'bun:ffi'
import {closeSync, mkdirSync, openSync} from 'node:fs'
import {dirname} from 'node:path'
import {DraftReviewError, EXIT} from './exit'

const LOCK_EX = 2
const LOCK_NB = 4
const POLL_MS = 25

const LIBC = process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6'

const libc = dlopen(LIBC, {flock: {args: [FFIType.i32, FFIType.i32], returns: FFIType.i32}})

const flock = (fd: number, operation: number): boolean => libc.symbols.flock(fd, operation) === 0

async function acquire(fd: number, lockPath: string, waitMs: number): Promise<void> {
  const deadline = performance.now() + waitMs
  while (!flock(fd, LOCK_EX | LOCK_NB)) {
    if (performance.now() >= deadline) throw new DraftReviewError(`another draft-review run holds ${lockPath}. This run waited ${waitMs} ms for it.`, EXIT.cantCreate)
    await Bun.sleep(POLL_MS)
  }
}

function openLockFile(lockPath: string): number {
  try {
    mkdirSync(dirname(lockPath), {recursive: true})
    return openSync(lockPath, 'a')
  } catch (error) {
    throw new DraftReviewError(`cannot open the lock file ${lockPath}: ${error instanceof Error ? error.message : String(error)}`, EXIT.cantCreate)
  }
}

// Closing the descriptor releases the flock, and the kernel also releases it if the process dies.
export async function withLock<Result>(lockPath: string, waitMs: number, action: () => Promise<Result>): Promise<Result> {
  const fd = openLockFile(lockPath)
  try {
    await acquire(fd, lockPath, waitMs)
    return await action()
  } finally {
    closeSync(fd)
  }
}
