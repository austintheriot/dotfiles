import {afterEach, describe, expect, test} from 'bun:test'
import {mkdtempSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {EXIT} from './config'
import {DEFAULT_PROFILE, expandHome, loadProfile, parseProfile, resolveProfilePath} from './profile'
import {VmError} from './proc'

const parsed = (json: unknown) => parseProfile(JSON.stringify(json))

const FULL = {
  version: 1,
  worktree: {paths: ['app', '.claude']},
  fonts: {ui: 'app/fonts/Ui.woff2', uiBold: 'app/fonts/UiBold.woff2', mono: 'app/fonts/Mono.woff2'},
  inputShims: ['.claude/skills/drive-desktop/cdp.mjs'],
  provision: {files: [{host: '~/.config/example/login.json', guest: '.config/example/login.json'}]},
  warm: {
    installs: [
      {dir: 'app', command: 'bun install', lockfile: 'app/bun.lock'},
      {dir: 'app/desktop', command: 'npm install', lockfile: 'app/desktop/package-lock.json', required: false}
    ],
    secretEnv: ['REGISTRY_TOKEN']
  },
  signin: {tabs: ['https://staging.example.com/app'], steps: ['Click "Sign in with Google" in the staging tab.']},
  session: {keepCookies: [{namePrefix: '_oauth2_proxy', domain: 'example.com'}]},
  chrome: {mediaOrigins: ['https://[*.]example.com']}
}

describe('parseProfile', () => {
  test('fills every section with its default when only the version is given', () => {
    expect(parsed({version: 1})).toEqual({ok: true, value: DEFAULT_PROFILE})
  })

  test('copies the whole checkout and adds nothing project-specific by default', () => {
    expect(DEFAULT_PROFILE.worktree.paths).toEqual(['.'])
    expect(DEFAULT_PROFILE.fonts).toBeUndefined()
    expect(DEFAULT_PROFILE.warm).toBeUndefined()
    expect(DEFAULT_PROFILE.inputShims).toEqual([])
    expect(DEFAULT_PROFILE.provision.files).toEqual([])
    expect(DEFAULT_PROFILE.chrome.mediaOrigins).toEqual([])
  })

  test('keeps every field of a full profile', () => {
    const result = parsed(FULL)
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.worktree.paths).toEqual(['app', '.claude'])
    expect(result.value.warm?.installs[0]?.required).toBe(true)
    expect(result.value.warm?.installs[1]?.required).toBe(false)
    expect(result.value.session.keepCookies).toEqual([{namePrefix: '_oauth2_proxy', domain: 'example.com'}])
  })

  test('refuses text that is not JSON', () => {
    expect(parseProfile('{')).toEqual({ok: false, reason: 'not JSON'})
  })

  test('refuses a version it does not know', () => {
    expect(parsed({version: 2})).toEqual({ok: false, reason: 'version: invalid_literal'})
  })

  test('names an unknown key, so a typo does not silently fall back to a default', () => {
    expect(parsed({version: 1, worktrees: {paths: ['app']}})).toEqual({ok: false, reason: ': unrecognized_keys worktrees'})
  })

  test('refuses worktree paths that leave the checkout', () => {
    expect(parsed({version: 1, worktree: {paths: ['../secrets']}}).ok).toBe(false)
    expect(parsed({version: 1, worktree: {paths: ['/etc']}}).ok).toBe(false)
    expect(parsed({version: 1, worktree: {paths: []}}).ok).toBe(false)
  })

  test('refuses a guest provision path outside the guest home', () => {
    expect(parsed({version: 1, provision: {files: [{host: '~/a', guest: '/etc/a'}]}}).ok).toBe(false)
  })

  test('refuses a secret name that is not an environment variable name', () => {
    const warm = {installs: [{dir: 'app', command: 'bun install', lockfile: 'app/bun.lock'}], secretEnv: ['TOKEN; rm -rf ~']}
    expect(parsed({version: 1, warm}).ok).toBe(false)
  })

  test('refuses a warm step with no installs', () => {
    expect(parsed({version: 1, warm: {installs: []}}).ok).toBe(false)
  })

  test('refuses a sign-in tab that is not a URL', () => {
    expect(parsed({version: 1, signin: {tabs: ['staging']}}).ok).toBe(false)
  })
})

describe('resolveProfilePath', () => {
  test('uses VM_PROFILE when it is set', () => {
    expect(resolveProfilePath({VM_PROFILE: '/x/profile.json'}, '/home/me')).toBe('/x/profile.json')
  })

  test('falls back to the machine-local profile', () => {
    expect(resolveProfilePath({}, '/home/me')).toBe('/home/me/.claude/local/vm/profile.json')
    expect(resolveProfilePath({VM_PROFILE: ''}, '/home/me')).toBe('/home/me/.claude/local/vm/profile.json')
  })
})

describe('expandHome', () => {
  test('expands a leading tilde only', () => {
    expect(expandHome('~/a/b', '/home/me')).toBe('/home/me/a/b')
    expect(expandHome('~', '/home/me')).toBe('/home/me')
    expect(expandHome('/a/~/b', '/home/me')).toBe('/a/~/b')
  })
})

describe('loadProfile', () => {
  const dirs: string[] = []
  afterEach(() => dirs.splice(0).forEach(dir => rmSync(dir, {recursive: true, force: true})))
  const file = (text: string) => {
    const dir = mkdtempSync(join(tmpdir(), 'profile-'))
    dirs.push(dir)
    const path = join(dir, 'profile.json')
    writeFileSync(path, text)
    return path
  }

  test('uses the defaults when no profile file exists', () => {
    expect(loadProfile({VM_PROFILE: '/nonexistent/profile.json'})).toEqual({
      profile: DEFAULT_PROFILE,
      source: {kind: 'defaults', path: '/nonexistent/profile.json'}
    })
  })

  test('reads the file VM_PROFILE names', () => {
    const path = file(JSON.stringify(FULL))
    const loaded = loadProfile({VM_PROFILE: path})
    expect(loaded.source).toEqual({kind: 'file', path})
    expect(loaded.profile.worktree.paths).toEqual(['app', '.claude'])
  })

  test('fails with the configuration exit code and the path when the file is invalid', () => {
    const path = file('{"version": 1, "worktree": {"paths": []}}')
    let caught: unknown
    try {
      loadProfile({VM_PROFILE: path})
    } catch (error) {
      caught = error
    }
    expect(caught).toBeInstanceOf(VmError)
    expect((caught as VmError).exitCode).toBe(EXIT.config)
    expect((caught as VmError).message).toContain(path)
  })
})
