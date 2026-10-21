import {describe, expect, test} from 'bun:test'
import type {CitedFile, Ledger} from './ledger'
import {EMPTY_LEDGER, planUploads, recordUpload} from './ledger'

const HASH_A = 'a'.repeat(64)
const HASH_B = 'b'.repeat(64)
const ledgerOf = (...entries: Ledger['entries']): Ledger => ({entries})
const present = (path: string, sha256: string): CitedFile => ({kind: 'present', path, sha256})
const gone = (path: string, recordedSha256: string | undefined): CitedFile => ({kind: 'gone', path, recordedSha256})

describe('planUploads', () => {
  test('uploads every cited file when the ledger is empty, in citation order', () => {
    const plan = planUploads([present('/q/b.png', HASH_B), present('/q/a.png', HASH_A)], EMPTY_LEDGER)
    expect(plan).toEqual({urlByPath: new Map(), toUpload: [{path: '/q/b.png', sha256: HASH_B}, {path: '/q/a.png', sha256: HASH_A}], missingFiles: []})
  })

  test('reuses the URL of a file whose content the ledger already holds', () => {
    const plan = planUploads([present('/q/a.png', HASH_A)], ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'}))
    expect(plan).toEqual({urlByPath: new Map([['/q/a.png', 'https://cdn.example/a']]), toUpload: [], missingFiles: []})
  })

  test('reuses the URL of the same content at a different path', () => {
    const plan = planUploads([present('/q/copy.png', HASH_A)], ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'}))
    expect(plan.urlByPath).toEqual(new Map([['/q/copy.png', 'https://cdn.example/a']]))
  })

  test('uploads again when the file at a known path has new content', () => {
    const plan = planUploads([present('/q/a.png', HASH_B)], ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'}))
    expect(plan.toUpload).toEqual([{path: '/q/a.png', sha256: HASH_B}])
  })

  test('resolves a gone file by the hash that add recorded', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'}, {path: '/q/a.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(planUploads([gone('/q/a.png', HASH_A)], ledger)).toEqual({urlByPath: new Map([['/q/a.png', 'https://cdn.example/a']]), toUpload: [], missingFiles: []})
  })

  test('reports a gone file whose recorded hash the ledger does not hold, even when the path has another URL', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'})
    expect(planUploads([gone('/q/a.png', HASH_B)], ledger)).toEqual({urlByPath: new Map(), toUpload: [], missingFiles: ['/q/a.png']})
  })

  test('reports a gone file with no recorded hash, even when the path has a URL', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'})
    expect(planUploads([gone('/q/a.png', undefined)], ledger)).toEqual({urlByPath: new Map(), toUpload: [], missingFiles: ['/q/a.png']})
  })

  test('plans one upload for two paths with the same new content', () => {
    const plan = planUploads([present('/q/a.png', HASH_A), present('/q/copy.png', HASH_A)], EMPTY_LEDGER)
    expect(plan.toUpload).toEqual([{path: '/q/a.png', sha256: HASH_A}])
  })

  test('after the upload is recorded, a second plan maps both paths and uploads nothing', () => {
    const cited = [present('/q/a.png', HASH_A), present('/q/copy.png', HASH_A)]
    const ledger = recordUpload(EMPTY_LEDGER, {path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'})
    expect(planUploads(cited, ledger)).toEqual({
      urlByPath: new Map([['/q/a.png', 'https://cdn.example/a'], ['/q/copy.png', 'https://cdn.example/a']]),
      toUpload: [],
      missingFiles: []
    })
  })
})

describe('recordUpload', () => {
  test('appends the entry and leaves the input ledger unchanged', () => {
    const before = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: 'https://cdn.example/a'})
    const after = recordUpload(before, {path: '/q/b.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(after.entries.map(entry => entry.path)).toEqual(['/q/a.png', '/q/b.png'])
    expect(before.entries).toHaveLength(1)
  })
})
