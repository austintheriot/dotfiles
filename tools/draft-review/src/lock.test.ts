import {expect, test} from 'bun:test'
import {mkdtempSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {DraftReviewError, EXIT} from './exit'
import {withLock} from './lock'

const lockPath = () => join(mkdtempSync(join(tmpdir(), 'draft-review-lock-')), '.lock')

test('runs the action and returns its result', async () => {
  expect(await withLock(lockPath(), 100, async () => 'done')).toBe('done')
})

test('refuses with exit 73 when the lock stays held past the wait', async () => {
  const path = lockPath()
  const caught = await withLock(path, 1000, () => withLock(path, 100, async () => 'second').catch((error: unknown) => error))
  expect(caught).toBeInstanceOf(DraftReviewError)
  expect(caught instanceof DraftReviewError ? caught.exitCode : undefined).toBe(EXIT.cantCreate)
})

test('releases the lock when the action throws', async () => {
  const path = lockPath()
  await expect(withLock(path, 100, async () => Promise.reject(new Error('action failed')))).rejects.toThrow('action failed')
  expect(await withLock(path, 100, async () => 'again')).toBe('again')
})
