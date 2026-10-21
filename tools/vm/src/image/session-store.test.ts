import {afterAll, describe, expect, test} from 'bun:test'
import {mkdtempSync, rmSync, statSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {run} from '../proc'
import {loadSession, parseKeyHex, saveSession, seal, unseal} from './session-store'

const key = Buffer.alloc(32, 7)
const otherKey = Buffer.alloc(32, 9)

describe('seal and unseal', () => {
  test('round-trips text', () => {
    expect(unseal(seal('hello', key), key)).toEqual({ok: true, value: 'hello'})
  })

  test('uses a fresh IV each time', () => {
    expect(seal('hello', key).equals(seal('hello', key))).toBe(false)
  })

  test('refuses the wrong key', () => {
    expect(unseal(seal('hello', key), otherKey)).toEqual({ok: false, reason: 'the key does not open it, or it was changed'})
  })

  test('refuses a changed byte', () => {
    const sealed = seal('hello', key)
    sealed[sealed.length - 1] = (sealed[sealed.length - 1] ?? 0) ^ 1
    expect(unseal(sealed, key).ok).toBe(false)
  })

  test('refuses a short or foreign file', () => {
    expect(unseal(Buffer.from('nope'), key)).toEqual({ok: false, reason: 'not a sealed session file'})
  })
})

describe('parseKeyHex', () => {
  test('reads 64 hex characters with a trailing newline', () => {
    expect(parseKeyHex(`${'ab'.repeat(32)}\n`)?.length).toBe(32)
  })

  test('refuses anything else', () => {
    expect(parseKeyHex('ab')).toBeUndefined()
    expect(parseKeyHex('zz'.repeat(32))).toBeUndefined()
  })
})

const keychain = process.env['VM_KEYCHAIN_TESTS'] === '1' ? describe : describe.skip

keychain('session store against the login keychain', () => {
  const item = {service: `vm-qa-test-key-${process.pid}`, account: 'vm-test'}
  const dir = mkdtempSync(join(tmpdir(), 'vm-session-'))
  const file = join(dir, 'nested', 'session.enc')
  const record = {version: 1 as const, exportedAt: '2026-10-14T10:00:00.000Z', account: 'vm-test', cookies: []}

  afterAll(() => {
    run(['security', 'delete-generic-password', '-s', item.service, '-a', item.account])
    rmSync(dir, {recursive: true, force: true})
  })

  test('reports a missing file as missing', () => {
    expect(loadSession(file, item)).toEqual({kind: 'missing'})
  })

  test('reports a file with no key as invalid', () => {
    writeFileSync(join(dir, 'orphan.enc'), seal('{}', key))
    expect(loadSession(join(dir, 'orphan.enc'), item).kind).toBe('invalid')
  })

  test('saves with mode 0600, then loads the same record', () => {
    saveSession(record, file, item)
    expect(statSync(file).mode & 0o777).toBe(0o600)
    expect(loadSession(file, item)).toEqual({kind: 'found', record})
  })
})
