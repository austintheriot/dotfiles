import {describe, expect, test} from 'bun:test'
import {readFileSync} from 'node:fs'
import {join} from 'node:path'
import {z} from 'zod'
import type {Finding, TraceVerdict} from './contribution'
import {
  checkContribution,
  CodeReviewShapeSchema,
  CodeTraceShapeSchema,
  ContributionCommentSchema,
  DevQaShapeSchema,
  DevQaVerifyShapeSchema,
  FindingSchema,
  notRun,
  TraceVerdictSchema,
  VerifyVerdictSchema
} from './contribution'
import {blankPage, devQa, HEAD, review, trace} from './testing/fixtures'
import type {Requirement} from './testing/posture'
import {requirementsByKind, singleRequirement, tableRows} from './testing/posture'

const DEFAULT_POSTURE = join(import.meta.dir, '..', '..', '..', '.claude', 'rules', 'outside-pr-review.md')
const readPosture = (path = process.env['DRAFT_REVIEW_POSTURE'] ?? DEFAULT_POSTURE) => readFileSync(path, 'utf8')

type ObjectSchema = z.ZodObject<z.ZodRawShape>

const quotedValues = (text: string) => [...text.matchAll(/`"([^"]+)"`/g)].map(match => match[1] ?? '')

const requirementIn = (schema: ObjectSchema, field: string): Requirement => {
  const member = schema.shape[field]
  if (member === undefined) return 'absent'
  return member.isOptional() ? 'optional' : 'required'
}

function enumValues(member: z.ZodTypeAny | undefined): string[] {
  if (member instanceof z.ZodOptional) return enumValues(member.unwrap())
  if (member instanceof z.ZodEnum) return [...member.options]
  if (member instanceof z.ZodLiteral && typeof member.value === 'string') return [member.value]
  return []
}

const fieldsOf = (schemas: readonly ObjectSchema[]) => [...new Set(schemas.flatMap(schema => Object.keys(schema.shape)))].sort()

function perKindValues(cell: string, kinds: readonly string[]): Record<string, string[]> {
  const parts = cell.split(/`([a-z-]+)`:/)
  return Object.fromEntries(
    parts.flatMap((part, index) => (kinds.includes(part) ? [[part, quotedValues(parts[index + 1] ?? '').sort()]] : []))
  )
}

const posture = readPosture()

describe('the top-level field table agrees with the four contribution shapes', () => {
  const kinds = ['code-review', 'dev-qa', 'dev-qa-verify', 'code-trace'] as const
  const schemas: Record<(typeof kinds)[number], ObjectSchema> = {
    'code-review': CodeReviewShapeSchema,
    'dev-qa': DevQaShapeSchema,
    'dev-qa-verify': DevQaVerifyShapeSchema,
    'code-trace': CodeTraceShapeSchema
  }
  const rows = tableRows(posture, '#### Top-level fields')

  test('names the same fields', () => {
    expect(rows.map(row => row.field).sort()).toEqual(fieldsOf(Object.values(schemas)))
  })

  test('gives each field the same required or optional status for each kind', () => {
    const fromTable = Object.fromEntries(rows.map(row => [row.field, requirementsByKind(row.required, kinds)]))
    const fromSchemas = Object.fromEntries(
      fieldsOf(Object.values(schemas)).map(field => [field, Object.fromEntries(kinds.map(kind => [kind, requirementIn(schemas[kind], field)]))])
    )
    expect(fromTable).toEqual(fromSchemas)
  })

  test('lists the same kind values', () => {
    const kindRow = rows.find(row => row.field === 'kind')
    expect(quotedValues(kindRow?.type ?? '').sort()).toEqual(kinds.flatMap(kind => enumValues(schemas[kind].shape['kind'])).sort())
  })

  test('lists the same status values for each kind', () => {
    const statusRow = rows.find(row => row.field === 'status')
    const fromSchemas = Object.fromEntries(
      kinds.flatMap(kind => {
        const values = enumValues(schemas[kind].shape['status'])
        return values.length === 0 ? [] : [[kind, values.sort()]]
      })
    )
    expect(perKindValues(statusRow?.type ?? '', kinds)).toEqual(fromSchemas)
  })
})

describe('the comment field table agrees with the comment shape', () => {
  const rows = tableRows(posture, '#### Comment fields')

  test('names the same fields with the same status', () => {
    const fromTable = Object.fromEntries(rows.map(row => [row.field, singleRequirement(row.required)]))
    const fromSchema = Object.fromEntries(Object.keys(ContributionCommentSchema.shape).map(field => [field, requirementIn(ContributionCommentSchema, field)]))
    expect(fromTable).toEqual(fromSchema)
  })

  test('lists the same side values', () => {
    const sideRow = rows.find(row => row.field === 'side')
    expect(quotedValues(sideRow?.type ?? '').sort()).toEqual(enumValues(ContributionCommentSchema.shape.side).sort())
  })
})

describe('the finding field table agrees with the finding shape', () => {
  const rows = tableRows(posture, '#### Finding fields')

  test('names the same fields with the same status', () => {
    const fromTable = Object.fromEntries(rows.map(row => [row.field, singleRequirement(row.required)]))
    const fromSchema = Object.fromEntries(Object.keys(FindingSchema.shape).map(field => [field, requirementIn(FindingSchema, field)]))
    expect(fromTable).toEqual(fromSchema)
  })

  test('lists the same finding kind values', () => {
    const kindRow = rows.find(row => row.field === 'kind')
    expect(quotedValues(kindRow?.type ?? '').sort()).toEqual(enumValues(FindingSchema.shape.kind).sort())
  })
})

describe('the verdict field table agrees with the two verdict shapes', () => {
  const kinds = ['dev-qa-verify', 'code-trace'] as const
  const schemas: Record<(typeof kinds)[number], ObjectSchema> = {'dev-qa-verify': VerifyVerdictSchema, 'code-trace': TraceVerdictSchema}
  const rows = tableRows(posture, '#### Verdict fields')

  test('names the same fields with the same status for each kind', () => {
    const fromTable = Object.fromEntries(rows.map(row => [row.field, requirementsByKind(row.required, kinds)]))
    const fromSchemas = Object.fromEntries(
      fieldsOf(Object.values(schemas)).map(field => [field, Object.fromEntries(kinds.map(kind => [kind, requirementIn(schemas[kind], field)]))])
    )
    expect(fromTable).toEqual(fromSchemas)
  })

  test('lists the same result values for each kind', () => {
    const resultRow = rows.find(row => row.field === 'result')
    const fromSchemas = Object.fromEntries(kinds.map(kind => [kind, enumValues(schemas[kind].shape['result']).sort()]))
    expect(perKindValues(resultRow?.meaning ?? '', kinds)).toEqual(fromSchemas)
  })
})

describe('the conditional rules in the tables hold in checkContribution', () => {
  const accepts = (value: unknown) => expect(checkContribution(value)).toMatchObject({ok: true})
  const rejects = (value: unknown, field: string) => {
    const result = checkContribution(value)
    expect(result.ok ? 'accepted' : result.reason).toContain(field)
  }
  const passed = () => {
    const {reason: _reason, ...withoutReason} = devQa({status: 'passed'})
    return withoutReason
  }
  const behaviorFinding = (overrides: Partial<Finding>): Finding => ({...blankPage(), ...overrides})
  const traceVerdict = (overrides: Partial<TraceVerdict>): TraceVerdict => ({id: 'Q1', result: 'root-cause', body: 'Same cause.', duplicateOf: 'R1', ...overrides})

  test('reason: a passed dev-qa run needs none, and any other status needs one', () => {
    accepts(passed())
    rejects({...passed(), status: 'failed'}, 'reason')
  })

  test('behavior and evidence: a behavior finding needs both', () => {
    accepts(devQa({findings: [behaviorFinding({})]}))
    const {behavior: _behavior, ...withoutBehavior} = behaviorFinding({})
    rejects(devQa({findings: [withoutBehavior]}), 'behavior')
    rejects(devQa({findings: [behaviorFinding({evidence: []})]}), 'evidence')
  })

  test('duplicateOf: allowed with root-cause and no-code-cause, refused with inconclusive', () => {
    accepts(trace([traceVerdict({result: 'root-cause'})]))
    accepts(trace([traceVerdict({result: 'no-code-cause'})]))
    rejects(trace([traceVerdict({result: 'inconclusive'})]), 'duplicateOf')
  })

  test('not-run: a not-run run has empty findings and comments', () => {
    accepts(notRun({kind: 'code-review', pr: 1234, head: HEAD, reason: 'The diff did not load.'}))
    accepts(notRun({kind: 'dev-qa', pr: 1234, head: HEAD, reason: 'No test machine.'}))
    rejects({...review(), status: 'not-run', reason: 'The diff did not load.'}, 'findings')
    rejects({...devQa({status: 'not-run', findings: []}), comments: [{path: 'a.ts', line: 1, side: 'RIGHT', body: 'x', finding: 'Q1'}]}, 'comments')
  })
})
