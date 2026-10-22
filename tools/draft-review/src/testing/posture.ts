export type Row = {field: string; type: string; required: string; meaning: string}
export type Requirement = 'required' | 'optional' | 'absent'

const cells = (line: string) =>
  line
    .trim()
    .replace(/^\||\|$/g, '')
    .split('|')
    .map(cell => cell.trim())

export function tableRows(markdown: string, heading: string): Row[] {
  const lines = markdown.split('\n')
  const start = lines.findIndex(line => line.trim() === heading)
  if (start < 0) throw new Error(`the posture file has no heading ${heading}`)
  const rest = lines.slice(start + 1)
  const tableStart = rest.findIndex(line => line.startsWith('|') || line.startsWith('#'))
  if (tableStart < 0 || !rest[tableStart]?.startsWith('|')) throw new Error(`the posture file has no table under ${heading}`)
  const afterTable = rest.slice(tableStart).findIndex(line => !line.startsWith('|'))
  const rows = rest.slice(tableStart, afterTable < 0 ? undefined : tableStart + afterTable).slice(2)
  if (rows.length === 0) throw new Error(`the table under ${heading} has no rows`)
  return rows.map(line => {
    const [field = '', type = '', required = '', meaning = ''] = cells(line)
    return {field: field.replace(/`/g, ''), type, required, meaning}
  })
}

const backticked = (text: string) => [...text.matchAll(/`([^`]+)`/g)].map(match => match[1] ?? '')

const KIND_CLAUSE = /^(required|optional)( and non-empty)? for (`[a-z-]+`(?: and `[a-z-]+`)?)( when .+)?$/
const NEUTRAL_CLAUSE = /^(otherwise optional|allowed with `[a-z-]+`(?: and `[a-z-]+`)?)$/

function clauseRequirements(clause: string, cell: string, kinds: readonly string[]): [string, Requirement][] {
  if (clause === 'required for all kinds') return kinds.map(kind => [kind, 'required'])
  if (NEUTRAL_CLAUSE.test(clause)) return []
  const match = KIND_CLAUSE.exec(clause)
  if (match === null) throw new Error(`the Required cell "${cell}" has a clause the agreement test does not know: "${clause}"`)
  const named = backticked(match[3] ?? '')
  const unknown = named.filter(name => !kinds.includes(name))
  if (unknown.length > 0) throw new Error(`the Required cell "${cell}" names kinds that do not exist: ${unknown.join(', ')}`)
  const status: Requirement = match[1] === 'required' && match[4] === undefined ? 'required' : 'optional'
  return named.map(kind => [kind, status])
}

export function requirementsByKind(cell: string, kinds: readonly string[]): Record<string, Requirement> {
  const named = Object.fromEntries(cell.split(', ').flatMap(clause => clauseRequirements(clause, cell, kinds)))
  return Object.fromEntries(kinds.map(kind => [kind, named[kind] ?? 'absent']))
}

const SINGLE_PHRASINGS: [RegExp, Requirement][] = [
  [/^required for every (comment|finding)$/, 'required'],
  [/^optional for every (comment|finding)$/, 'optional'],
  [/^required( and non-empty)? for `behavior`, optional for `code`$/, 'optional']
]

export function singleRequirement(cell: string): Requirement {
  const phrasing = SINGLE_PHRASINGS.find(([pattern]) => pattern.test(cell))
  if (phrasing === undefined) throw new Error(`the Required cell "${cell}" is not a phrasing the agreement test knows`)
  return phrasing[1]
}
