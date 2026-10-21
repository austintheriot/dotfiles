import {describe, expect, test} from 'bun:test'
import {requirementsByKind, singleRequirement, tableRows} from './posture'

const KINDS = ['code-review', 'dev-qa'] as const

describe('tableRows', () => {
  test('reads the rows of the table under the heading', () => {
    const markdown = '#### Fields\n\n| Field | Type | Required | Meaning |\n|---|---|---|---|\n| `id` | string | required for every finding | The id. |\n\nAfter.'
    expect(tableRows(markdown, '#### Fields')).toEqual([{field: 'id', type: 'string', required: 'required for every finding', meaning: 'The id.'}])
  })

  test('throws when the heading is missing', () => {
    expect(() => tableRows('#### Other\n\n| a |\n|---|\n| b |', '#### Fields')).toThrow('#### Fields')
  })

  test('throws when the heading has no table before the next heading', () => {
    expect(() => tableRows('#### Fields\n\nNo table here.\n\n#### Next\n\n| a |\n|---|\n| b |', '#### Fields')).toThrow('no table')
  })

  test('throws when the table has no rows', () => {
    expect(() => tableRows('#### Fields\n\n| Field | Required |\n|---|---|\n', '#### Fields')).toThrow('no rows')
  })
})

describe('requirementsByKind', () => {
  test('reads every known phrasing', () => {
    expect(requirementsByKind('required for all kinds', KINDS)).toEqual({'code-review': 'required', 'dev-qa': 'required'})
    expect(requirementsByKind('required for `code-review` and `dev-qa`', KINDS)).toEqual({'code-review': 'required', 'dev-qa': 'required'})
    expect(requirementsByKind('optional for `dev-qa`', KINDS)).toEqual({'code-review': 'absent', 'dev-qa': 'optional'})
    expect(requirementsByKind('required for `dev-qa`, optional for `code-review`', KINDS)).toEqual({'code-review': 'optional', 'dev-qa': 'required'})
    expect(requirementsByKind('required for `dev-qa` when `status` is not `passed`, otherwise optional', KINDS)).toEqual({'code-review': 'absent', 'dev-qa': 'optional'})
    expect(requirementsByKind('optional for `dev-qa`, allowed with `root-cause` and `no-code-cause`', KINDS)).toEqual({'code-review': 'absent', 'dev-qa': 'optional'})
  })

  test('throws on garbage text, naming the text', () => {
    expect(() => requirementsByKind('zebra banana quux', KINDS)).toThrow('zebra banana quux')
  })

  test('throws on a known phrasing that names an unknown kind', () => {
    expect(() => requirementsByKind('required for `dev-qa-typo`', KINDS)).toThrow('dev-qa-typo')
  })
})

describe('singleRequirement', () => {
  test('reads every known phrasing', () => {
    expect(singleRequirement('required for every comment')).toBe('required')
    expect(singleRequirement('required for every finding')).toBe('required')
    expect(singleRequirement('optional for every finding')).toBe('optional')
    expect(singleRequirement('required for `behavior`, optional for `code`')).toBe('optional')
    expect(singleRequirement('required and non-empty for `behavior`, optional for `code`')).toBe('optional')
  })

  test('throws on garbage text, naming the text', () => {
    expect(() => singleRequirement('zebra banana quux')).toThrow('zebra banana quux')
  })
})
