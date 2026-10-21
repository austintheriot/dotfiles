import {existsSync, readFileSync} from 'node:fs'
import {homedir} from 'node:os'
import {isAbsolute, join} from 'node:path'
import {z} from 'zod'
import {EXIT} from './config'
import type {Parsed} from './image/session'
import {VmError} from './proc'

const InsidePath = z
  .string()
  .min(1)
  .refine(path => !isAbsolute(path) && !path.startsWith('~') && !path.split('/').includes('..'), 'must be a relative path that stays inside its root')

const EnvName = z.string().regex(/^[A-Za-z_][A-Za-z0-9_]*$/, 'must be an environment variable name')

const Install = z
  .object({dir: InsidePath, command: z.string().min(1), lockfile: InsidePath, required: z.boolean().default(true)})
  .strict()

// Every section is optional, and its default adds nothing project-specific: the whole checkout goes into ~/wt.
export const Profile = z
  .object({
    version: z.literal(1),
    worktree: z.object({paths: z.array(InsidePath).min(1)}).strict().default({paths: ['.']}),
    fonts: z.object({ui: InsidePath, uiBold: InsidePath, mono: InsidePath}).strict().optional(),
    inputShims: z.array(InsidePath).default([]),
    provision: z
      .object({files: z.array(z.object({host: z.string().min(1), guest: InsidePath}).strict()).default([])})
      .strict()
      .default({}),
    warm: z
      .object({installs: z.array(Install).min(1), secretEnv: z.array(EnvName).default([])})
      .strict()
      .optional(),
    signin: z
      .object({tabs: z.array(z.string().url()).default([]), steps: z.array(z.string().min(1)).default([])})
      .strict()
      .default({}),
    session: z
      .object({keepCookies: z.array(z.object({namePrefix: z.string().min(1), domain: z.string().min(1)}).strict()).default([])})
      .strict()
      .default({}),
    chrome: z.object({mediaOrigins: z.array(z.string().min(1)).default([])}).strict().default({})
  })
  .strict()
export type Profile = z.infer<typeof Profile>
export type WarmConfig = NonNullable<Profile['warm']>
export type KeepCookie = Profile['session']['keepCookies'][number]

export const DEFAULT_PROFILE: Profile = Profile.parse({version: 1})

export function parseProfile(text: string): Parsed<Profile> {
  let json: unknown
  try {
    json = JSON.parse(text)
  } catch {
    return {ok: false, reason: 'not JSON'}
  }
  const parsed = Profile.safeParse(json)
  if (parsed.success) return {ok: true, value: parsed.data}
  const issue = parsed.error.issues[0]
  if (issue === undefined) return {ok: false, reason: 'invalid profile'}
  const keys = issue.code === 'unrecognized_keys' ? ` ${issue.keys.join(', ')}` : ''
  return {ok: false, reason: `${issue.path.join('.')}: ${issue.code}${keys}`}
}

export const expandHome = (path: string, home: string): string => (path === '~' ? home : path.startsWith('~/') ? join(home, path.slice(2)) : path)

export function resolveProfilePath(env: Record<string, string | undefined>, home: string): string {
  const named = env['VM_PROFILE']
  return named === undefined || named === '' ? join(home, '.claude/local/vm/profile.json') : expandHome(named, home)
}

export type ProfileSource = {kind: 'file' | 'defaults'; path: string}
export type ProfileStatus = ProfileSource | {kind: 'invalid'; path: string; reason: string}
export type LoadedProfile = {profile: Profile; source: ProfileSource}

function readProfile(env: Record<string, string | undefined>, home: string): LoadedProfile | {kind: 'invalid'; path: string; reason: string} {
  const path = resolveProfilePath(env, home)
  if (!existsSync(path)) return {profile: DEFAULT_PROFILE, source: {kind: 'defaults', path}}
  const parsed = parseProfile(readFileSync(path, 'utf8'))
  return parsed.ok ? {profile: parsed.value, source: {kind: 'file', path}} : {kind: 'invalid', path, reason: parsed.reason}
}

export function profileStatus(env: Record<string, string | undefined>, home: string = homedir()): ProfileStatus {
  const read = readProfile(env, home)
  return 'source' in read ? read.source : read
}

export function loadProfile(env: Record<string, string | undefined>, home: string = homedir()): LoadedProfile {
  const read = readProfile(env, home)
  if ('source' in read) return read
  throw new VmError(`the vm profile ${read.path} is invalid: ${read.reason}`, EXIT.config)
}
