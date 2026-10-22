import {describe, expect, test} from 'bun:test'
import type {CitedFile, Ledger} from './ledger'
import {EMPTY_LEDGER, planUploads, recordUpload} from './ledger'

const HASH_A = 'a'.repeat(64)
const HASH_B = 'b'.repeat(64)
const ledgerOf = (...entries: Ledger['entries']): Ledger => ({entries})
const unrecorded = (path: string, observedSha256: string | undefined): CitedFile => ({path, observedSha256, recorded: {kind: 'none'}})
const recorded = (path: string, observedSha256: string | undefined, sha256: string): CitedFile => ({path, observedSha256, recorded: {kind: 'recorded', sha256}})
const conflicting = (path: string, observedSha256: string | undefined): CitedFile => ({path, observedSha256, recorded: {kind: 'conflict', hashes: [HASH_A, HASH_B]}})
const URL_A = 'https://cdn.example/a'
const LEDGER_A = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: URL_A})

describe('planUploads with no recorded hash', () => {
  test('uploads every cited file when the ledger is empty, in citation order', () => {
    const plan = planUploads([unrecorded('/q/b.png', HASH_B), unrecorded('/q/a.png', HASH_A)], EMPTY_LEDGER)
    expect(plan).toEqual({urlByPath: new Map(), toUpload: [{path: '/q/b.png', sha256: HASH_B}, {path: '/q/a.png', sha256: HASH_A}], unresolved: []})
  })

  test('reuses the URL of a file whose content the ledger already holds', () => {
    expect(planUploads([unrecorded('/q/a.png', HASH_A)], LEDGER_A)).toEqual({urlByPath: new Map([['/q/a.png', URL_A]]), toUpload: [], unresolved: []})
  })

  test('reuses the URL of the same content at a different path', () => {
    expect(planUploads([unrecorded('/q/copy.png', HASH_A)], LEDGER_A).urlByPath).toEqual(new Map([['/q/copy.png', URL_A]]))
  })

  test('uploads again when the file at a known path has new content', () => {
    expect(planUploads([unrecorded('/q/a.png', HASH_B)], LEDGER_A).toUpload).toEqual([{path: '/q/a.png', sha256: HASH_B}])
  })

  test('reports a gone file, even when the path has a URL', () => {
    expect(planUploads([unrecorded('/q/a.png', undefined)], LEDGER_A)).toEqual({urlByPath: new Map(), toUpload: [], unresolved: [{path: '/q/a.png', reason: 'gone'}]})
  })

  test('plans one upload for two paths with the same new content', () => {
    expect(planUploads([unrecorded('/q/a.png', HASH_A), unrecorded('/q/copy.png', HASH_A)], EMPTY_LEDGER).toUpload).toEqual([{path: '/q/a.png', sha256: HASH_A}])
  })

  test('after the upload is recorded, a second plan maps both paths and uploads nothing', () => {
    const cited = [unrecorded('/q/a.png', HASH_A), unrecorded('/q/copy.png', HASH_A)]
    const ledger = recordUpload(EMPTY_LEDGER, {path: '/q/a.png', sha256: HASH_A, url: URL_A})
    expect(planUploads(cited, ledger)).toEqual({urlByPath: new Map([['/q/a.png', URL_A], ['/q/copy.png', URL_A]]), toUpload: [], unresolved: []})
  })
})

describe('planUploads with the hash that add recorded', () => {
  test('uploads a present file whose content matches the recorded hash', () => {
    expect(planUploads([recorded('/q/a.png', HASH_A, HASH_A)], EMPTY_LEDGER)).toEqual({urlByPath: new Map(), toUpload: [{path: '/q/a.png', sha256: HASH_A}], unresolved: []})
  })

  test('reuses the URL of a present file whose content matches the recorded hash', () => {
    expect(planUploads([recorded('/q/a.png', HASH_A, HASH_A)], LEDGER_A)).toEqual({urlByPath: new Map([['/q/a.png', URL_A]]), toUpload: [], unresolved: []})
  })

  test('uses the ledger URL of the recorded content when the file changed after add, and uploads nothing', () => {
    expect(planUploads([recorded('/q/a.png', HASH_B, HASH_A)], LEDGER_A)).toEqual({urlByPath: new Map([['/q/a.png', URL_A]]), toUpload: [], unresolved: []})
  })

  test('reports a file that changed after add when the ledger has no upload of the recorded content', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(planUploads([recorded('/q/a.png', HASH_B, HASH_A)], ledger)).toEqual({urlByPath: new Map(), toUpload: [], unresolved: [{path: '/q/a.png', reason: 'changed'}]})
  })

  test('resolves a gone file by the recorded hash', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: URL_A}, {path: '/q/a.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(planUploads([recorded('/q/a.png', undefined, HASH_A)], ledger)).toEqual({urlByPath: new Map([['/q/a.png', URL_A]]), toUpload: [], unresolved: []})
  })

  test('reports a gone file whose recorded hash the ledger does not hold, even when the path has another URL', () => {
    expect(planUploads([recorded('/q/a.png', undefined, HASH_B)], LEDGER_A)).toEqual({urlByPath: new Map(), toUpload: [], unresolved: [{path: '/q/a.png', reason: 'gone'}]})
  })

  test('reports a path for which two contributions recorded different hashes, whatever the file and the ledger hold', () => {
    const ledger = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: URL_A}, {path: '/q/a.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(planUploads([conflicting('/q/a.png', HASH_A), conflicting('/q/b.png', undefined)], ledger)).toEqual({
      urlByPath: new Map(),
      toUpload: [],
      unresolved: [
        {path: '/q/a.png', reason: 'conflict'},
        {path: '/q/b.png', reason: 'conflict'}
      ]
    })
  })
})

describe('recordUpload', () => {
  test('appends the entry and leaves the input ledger unchanged', () => {
    const before = ledgerOf({path: '/q/a.png', sha256: HASH_A, url: URL_A})
    const after = recordUpload(before, {path: '/q/b.png', sha256: HASH_B, url: 'https://cdn.example/b'})
    expect(after.entries.map(entry => entry.path)).toEqual(['/q/a.png', '/q/b.png'])
    expect(before.entries).toHaveLength(1)
  })
})
