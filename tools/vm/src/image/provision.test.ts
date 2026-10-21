import {afterEach, describe, expect, test} from 'bun:test'
import {mkdtempSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {EXIT} from '../config'
import {VmError} from '../proc'
import {readProvisionFiles} from './provision'

const dirs: string[] = []
afterEach(() => dirs.splice(0).forEach(dir => rmSync(dir, {recursive: true, force: true})))
function home(files: Record<string, string>): string {
  const dir = mkdtempSync(join(tmpdir(), 'provision-'))
  dirs.push(dir)
  Object.entries(files).forEach(([name, content]) => writeFileSync(join(dir, name), content))
  return dir
}

describe('readProvisionFiles', () => {
  test('reads each host file, expanding a leading tilde, and keeps its guest path', () => {
    const dir = home({'login.json': '{"a":1}'})
    expect(readProvisionFiles([{host: '~/login.json', guest: '.config/example/login.json'}], dir)).toEqual([
      {guest: '.config/example/login.json', content: '{"a":1}'}
    ])
  })

  test('needs nothing when the profile lists no files', () => {
    expect(readProvisionFiles([], home({}))).toEqual([])
  })

  test('refuses a missing or empty host file with the missing-input exit code', () => {
    const dir = home({'empty.json': ''})
    for (const host of ['~/absent.json', '~/empty.json']) {
      let caught: unknown
      try {
        readProvisionFiles([{host, guest: 'x'}], dir)
      } catch (error) {
        caught = error
      }
      expect(caught).toBeInstanceOf(VmError)
      expect((caught as VmError).exitCode).toBe(EXIT.noInput)
    }
  })
})
