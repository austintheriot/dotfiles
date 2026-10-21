import {afterEach, describe, expect, test} from 'bun:test'
import {mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import type {WarmConfig} from '../profile'
import {dueInstalls, warmInputs, warmScript} from './warm'

const dirs: string[] = []
function checkout(files: Record<string, string>): string {
  const dir = mkdtempSync(join(tmpdir(), 'warm-'))
  dirs.push(dir)
  for (const [name, content] of Object.entries(files)) {
    mkdirSync(join(dir, name, '..'), {recursive: true})
    writeFileSync(join(dir, name), content)
  }
  return dir
}
afterEach(() => dirs.splice(0).forEach(dir => rmSync(dir, {recursive: true, force: true})))

const WARM: WarmConfig = {
  installs: [
    {dir: 'app', command: 'bun install', lockfile: 'app/bun.lock', required: true},
    {dir: 'app/desktop', command: 'npm install --no-audit', lockfile: 'app/desktop/package-lock.json', required: false}
  ],
  secretEnv: ['REGISTRY_TOKEN']
}

describe('warmInputs', () => {
  test('refuses a checkout without a required lockfile', () => {
    expect(() => warmInputs(checkout({'app/package.json': '{}'}), WARM)).toThrow('no app/bun.lock')
  })

  test('hashes the same contents to the same value', () => {
    expect(warmInputs(checkout({'app/bun.lock': 'a'}), WARM)).toEqual(warmInputs(checkout({'app/bun.lock': 'a'}), WARM))
  })

  test('changes when an optional lockfile appears or changes, and keeps the other hashes', () => {
    const files = {'app/bun.lock': 'a'}
    const without = warmInputs(checkout(files), WARM)
    const first = warmInputs(checkout({...files, 'app/desktop/package-lock.json': 'x'}), WARM)
    const second = warmInputs(checkout({...files, 'app/desktop/package-lock.json': 'y'}), WARM)
    expect(first).not.toEqual(without)
    expect(first).not.toEqual(second)
    expect(first['app/bun.lock']).toBe(without['app/bun.lock'])
  })
})

describe('dueInstalls', () => {
  test('skips an optional install whose lockfile the checkout lacks', () => {
    expect(dueInstalls(checkout({'app/bun.lock': 'a'}), WARM).map(install => install.dir)).toEqual(['app'])
  })

  test('runs every install whose lockfile exists', () => {
    const dir = checkout({'app/bun.lock': 'a', 'app/desktop/package-lock.json': 'x'})
    expect(dueInstalls(dir, WARM).map(install => install.dir)).toEqual(['app', 'app/desktop'])
  })
})

describe('warmScript', () => {
  test('reads each secret from stdin and runs each install in its directory under ~/warm', () => {
    const script = warmScript(WARM.installs, ['REGISTRY_TOKEN'])
    expect(script).toContain('read -r REGISTRY_TOKEN && export REGISTRY_TOKEN')
    expect(script).toContain("(cd ~/warm/'app' && bun install 2>&1 | tail -3)")
    expect(script).toContain("(cd ~/warm/'app/desktop' && npm install --no-audit 2>&1 | tail -3)")
    expect(script.indexOf('read -r')).toBeLessThan(script.indexOf('bun install'))
  })

  test('quotes a directory with a quote in it', () => {
    expect(warmScript([{dir: "it's", command: 'make', lockfile: 'x', required: true}], [])).toContain(`(cd ~/warm/'it'\\''s' && make 2>&1 | tail -3)`)
  })
})
