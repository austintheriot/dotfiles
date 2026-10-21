import {homedir} from 'node:os'
import {join} from 'node:path'

export const BASE_IMAGE = 'pr-qa-base'
export const PULL_DIR = '/tmp/pr-review-qa'
export const FONTS_DIR = join(PULL_DIR, 'fonts')
export const CHROME_PORT = 9333
export const VM_SCREEN = {width: 1680, height: 1050} as const
export const LEASE_DIR = join(homedir(), '.cache/pr-review/vm-leases')
export const VIDEO_DIR = join(import.meta.dir, '..', 'video')

// The base image's admin account. The recorder and the input proxy run there.
export const VM_BUN = '/Users/admin/.bun/bin/bun'
export const VM_VIDEO_TOOLS = '/Users/admin/qa-video'
export const VM_TOOL_FILES = [
  'core.ts',
  'timeline.ts',
  'proxy.ts',
  'cli.ts',
  'recording-state.ts',
  'cdp-timeline.ts',
  'cdp-log.ts',
  'cdp-shim.mjs',
  'os-input.ts',
  'os-shell.ts'
]

export const SESSION_FILE = join(homedir(), '.config/vm/google-session.enc')
export const SESSION_KEY_SERVICE = 'vm-qa-google-session-key'
export const IMAGE_STATE_FILE = join(homedir(), '.cache/pr-review/vm-image-state.json')
export const GUEST_DIR = join(import.meta.dir, '..', 'guest')
export const VM_GUEST_TOOLS = '/Users/admin/qa-guest'
export const GUEST_TOOL_FILES = ['cdp-session.ts']
export const ELECTRON_PORT = 9224

export const vmName = (pr: number) => `pr-qa-${pr}`

// BSD exit codes from sysexits.h, which schedulers and the docs rely on.
export const EXIT = {usage: 64, noInput: 66, unavailable: 69, inUse: 73, tempFail: 75, config: 78} as const
