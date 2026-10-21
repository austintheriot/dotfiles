import {createCipheriv, createDecipheriv, randomBytes} from 'node:crypto'
import {existsSync, mkdirSync, readFileSync, renameSync, writeFileSync} from 'node:fs'
import {userInfo} from 'node:os'
import {dirname} from 'node:path'
import {EXIT, SESSION_FILE, SESSION_KEY_SERVICE} from '../config'
import {run, VmError} from '../proc'
import {parseSessionRecord, type Parsed, type SessionRecord} from './session'

const FORMAT = 1
const IV_BYTES = 12
const TAG_BYTES = 16
const HEADER_BYTES = 1 + IV_BYTES + TAG_BYTES

export function seal(plaintext: string, key: Buffer, iv: Buffer = randomBytes(IV_BYTES)): Buffer {
  const cipher = createCipheriv('aes-256-gcm', key, iv)
  const body = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()])
  return Buffer.concat([Buffer.from([FORMAT]), iv, cipher.getAuthTag(), body])
}

export function unseal(sealed: Buffer, key: Buffer): Parsed<string> {
  if (sealed.length < HEADER_BYTES || sealed[0] !== FORMAT) return {ok: false, reason: 'not a sealed session file'}
  try {
    const decipher = createDecipheriv('aes-256-gcm', key, sealed.subarray(1, 1 + IV_BYTES))
    decipher.setAuthTag(sealed.subarray(1 + IV_BYTES, HEADER_BYTES))
    return {ok: true, value: Buffer.concat([decipher.update(sealed.subarray(HEADER_BYTES)), decipher.final()]).toString('utf8')}
  } catch {
    return {ok: false, reason: 'the key does not open it, or it was changed'}
  }
}

export function parseKeyHex(text: string): Buffer | undefined {
  const hex = text.trim()
  return /^[0-9a-f]{64}$/.test(hex) ? Buffer.from(hex, 'hex') : undefined
}

export type KeychainItem = {service: string; account: string}
export const sessionKeyItem = (): KeychainItem => ({service: SESSION_KEY_SERVICE, account: userInfo().username})

const ITEM_NOT_FOUND = 44
const PLAIN_NAME = /^[A-Za-z0-9._-]+$/

export function readKey(item: KeychainItem): Buffer | undefined {
  const result = run(['security', 'find-generic-password', '-s', item.service, '-a', item.account, '-w'])
  if (result.code === ITEM_NOT_FOUND) return undefined
  if (result.code !== 0) throw new VmError(`could not read the Keychain item ${item.service}: ${result.stderr.trim()}`, EXIT.unavailable)
  const key = parseKeyHex(result.stdout)
  if (key === undefined) throw new VmError(`the Keychain item ${item.service} does not hold a 32-byte hex key`, EXIT.unavailable)
  return key
}

// security -w <value> would put the key in argv, so the command goes in on stdin. security -i splits
// long lines, which is why only this short key, not the session, lives in the Keychain.
export function createKey(item: KeychainItem): Buffer {
  if (!PLAIN_NAME.test(item.service) || !PLAIN_NAME.test(item.account)) throw new VmError(`refusing Keychain name ${item.service} / ${item.account}`, EXIT.usage)
  const key = randomBytes(32)
  run(['security', '-i'], {stdin: `add-generic-password -U -s ${item.service} -a ${item.account} -T /usr/bin/security -w ${key.toString('hex')}\n`})
  if (readKey(item)?.equals(key) !== true) throw new VmError(`could not write the Keychain item ${item.service}`, EXIT.unavailable)
  return key
}

export type StoredSession = {kind: 'missing'} | {kind: 'invalid'; reason: string} | {kind: 'found'; record: SessionRecord}

export function loadSession(file: string = SESSION_FILE, item: KeychainItem = sessionKeyItem()): StoredSession {
  if (!existsSync(file)) return {kind: 'missing'}
  const key = readKey(item)
  if (key === undefined) return {kind: 'invalid', reason: `${file} exists, but the Keychain has no ${item.service} item`}
  const text = unseal(readFileSync(file), key)
  if (!text.ok) return {kind: 'invalid', reason: `${file}: ${text.reason}`}
  const record = parseSessionRecord(text.value)
  return record.ok ? {kind: 'found', record: record.value} : {kind: 'invalid', reason: `${file}: ${record.reason}`}
}

export function saveSession(record: SessionRecord, file: string = SESSION_FILE, item: KeychainItem = sessionKeyItem()): void {
  const key = readKey(item) ?? createKey(item)
  mkdirSync(dirname(file), {recursive: true, mode: 0o700})
  const temporary = `${file}.${process.pid}.tmp`
  writeFileSync(temporary, seal(JSON.stringify(record), key), {mode: 0o600})
  renameSync(temporary, file)
}
