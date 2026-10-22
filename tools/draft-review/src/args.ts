import {parseArgs} from 'node:util'
import type {Parsed} from './parsed'

export type Repo = {owner: string; name: string}

type Common = {repo: Repo | undefined; store: string | undefined}
type Target = {pr: number; head: string}

export type AddCommand = {kind: 'add'; file: string} & Common
export type RouteCommand = {kind: 'route'; out: string} & Target & Common
export type ShowCommand = {kind: 'show'; config: string | undefined} & Target & Common
export type CreateCommand = {kind: 'create'; config: string | undefined} & Target & Common
export type ClearCommand = {kind: 'clear'} & Target & Common
export type Command = {kind: 'help'} | AddCommand | RouteCommand | ShowCommand | CreateCommand | ClearCommand

export const USAGE = `Usage:
  draft-review add <contribution.json> [--repo <owner/repo>] [--store <dir>]
  draft-review route --pr <n> --head <sha> --out <dir> [--repo <owner/repo>] [--store <dir>]
  draft-review show --pr <n> --head <sha> [--repo <owner/repo>] [--store <dir>] [--config <file>]
  draft-review create --pr <n> --head <sha> [--repo <owner/repo>] [--store <dir>] [--config <file>]
  draft-review clear --pr <n> --head <sha> [--repo <owner/repo>] [--store <dir>]`

const NAME = /^[A-Za-z0-9_.-]+$/

export function parseRepo(text: string): Parsed<Repo> {
  const parts = text.split('/')
  const [owner, name] = parts
  const valid = (part: string | undefined): part is string => part !== undefined && NAME.test(part) && part !== '.' && part !== '..'
  if (parts.length !== 2 || !valid(owner) || !valid(name)) return {ok: false, reason: `--repo must be <owner>/<repo>, got ${text}`}
  return {ok: true, value: {owner, name}}
}

const OPTIONS = {
  repo: {type: 'string'},
  store: {type: 'string'},
  pr: {type: 'string'},
  head: {type: 'string'},
  out: {type: 'string'},
  config: {type: 'string'}
} as const

type OptionName = keyof typeof OPTIONS
type Values = Partial<Record<OptionName, string>>

const ALLOWED: Record<Exclude<Command['kind'], 'help'>, readonly OptionName[]> = {
  add: ['repo', 'store'],
  route: ['pr', 'head', 'out', 'repo', 'store'],
  show: ['pr', 'head', 'repo', 'store', 'config'],
  create: ['pr', 'head', 'repo', 'store', 'config'],
  clear: ['pr', 'head', 'repo', 'store']
}

const isCommandName = (name: string): name is keyof typeof ALLOWED => Object.hasOwn(ALLOWED, name)

function readOptions(args: readonly string[]): Parsed<{values: Values; positionals: string[]}> {
  try {
    const {values, positionals} = parseArgs({args: [...args], options: OPTIONS, allowPositionals: true, strict: true})
    return {ok: true, value: {values, positionals}}
  } catch (error) {
    return {ok: false, reason: error instanceof Error ? error.message : String(error)}
  }
}

function common(values: Values): Parsed<Common> {
  if (values.repo === undefined) return {ok: true, value: {repo: undefined, store: values.store}}
  const repo = parseRepo(values.repo)
  return repo.ok ? {ok: true, value: {repo: repo.value, store: values.store}} : repo
}

function target(values: Values): Parsed<Target> {
  if (values.pr === undefined) return {ok: false, reason: '--pr is required'}
  if (!/^[1-9][0-9]*$/.test(values.pr)) return {ok: false, reason: `--pr must be a positive whole number, got ${values.pr}`}
  if (values.head === undefined) return {ok: false, reason: '--head is required'}
  if (!/^[0-9a-f]{40}$/.test(values.head)) return {ok: false, reason: `--head must be a full 40-character commit SHA, got ${values.head}`}
  return {ok: true, value: {pr: Number(values.pr), head: values.head}}
}

function build(kind: keyof typeof ALLOWED, values: Values, positionals: readonly string[]): Parsed<Command> {
  const shared = common(values)
  if (!shared.ok) return shared
  if (kind === 'add') {
    const [file] = positionals
    if (positionals.length !== 1 || file === undefined) return {ok: false, reason: 'add takes one contribution file'}
    return {ok: true, value: {kind, file, ...shared.value}}
  }
  const [extra] = positionals
  if (extra !== undefined) return {ok: false, reason: `${kind} takes no file argument, got ${extra}`}
  const aim = target(values)
  if (!aim.ok) return aim
  if (kind === 'route') {
    if (values.out === undefined) return {ok: false, reason: '--out is required'}
    return {ok: true, value: {kind, out: values.out, ...aim.value, ...shared.value}}
  }
  if (kind === 'clear') return {ok: true, value: {kind, ...aim.value, ...shared.value}}
  return {ok: true, value: {kind, config: values.config, ...aim.value, ...shared.value}}
}

export function parseCommand(argv: readonly string[]): Parsed<Command> {
  const [name, ...rest] = argv
  if (name === undefined || name === 'help' || name === '--help' || name === '-h') return {ok: true, value: {kind: 'help'}}
  if (!isCommandName(name)) return {ok: false, reason: `unknown command ${name}`}
  const options = readOptions(rest)
  if (!options.ok) return options
  const refused = Object.keys(options.value.values).find(option => !ALLOWED[name].some(allowed => allowed === option))
  if (refused !== undefined) return {ok: false, reason: `${name} does not take --${refused}`}
  return build(name, options.value.values, options.value.positionals)
}
