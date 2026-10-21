import {afterAll, describe, expect, test} from 'bun:test'
import {chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {bodyHash, draftAction, draftRecordAfterCreate} from './draft'
import type {DraftRecord, PendingReview, RecordedReview, SubmittedReview} from './draft'
import {DraftReviewError, EXIT} from './exit'
import {withLock} from './lock'
import type {IUploader} from './store'
import type {PrStore} from './store'
import {headDigest, prepareUploads, prStore, readContributions, readDraftRecord, readHead, runUploads, writeDraftRecord, writeReviewRequest, writeSynthesized} from './store'
import type {DevQaContribution} from './contribution'
import {blankPage, devQa, HEAD, OTHER_HEAD, payloadOf, review, trace, verify} from './testing/fixtures'
import {substituteUploads} from './uploads'

const MAIN = join(import.meta.dir, 'main.ts')
const REPO = {owner: 'acme', name: 'widgets'}
const worlds: string[] = []

afterAll(() => worlds.forEach(dir => rmSync(dir, {recursive: true, force: true})))

type Run = {code: number; stdout: string; stderr: string}

const FAKE_GH_MESSAGE = 'fake gh: the CLI tests never reach the real GitHub CLI'

function blockedGhEnv(dir: string): Record<string, string | undefined> {
  const bin = join(dir, 'fake-bin')
  mkdirSync(bin)
  const fakeGh = join(bin, 'gh')
  writeFileSync(fakeGh, `#!/bin/sh\necho "${FAKE_GH_MESSAGE}" >&2\nexit 1\n`)
  chmodSync(fakeGh, 0o755)
  const ghConfigDir = join(dir, 'gh-config')
  mkdirSync(ghConfigDir)
  return {...process.env, NO_COLOR: '1', PATH: `${bin}:${process.env.PATH ?? ''}`, GH_CONFIG_DIR: ghConfigDir, GH_TOKEN: 'invalid'}
}

function makeWorld() {
  const dir = mkdtempSync(join(tmpdir(), 'draft-review-test-'))
  worlds.push(dir)
  const env = blockedGhEnv(dir)
  const root = join(dir, 'store')
  const store = prStore(root, REPO, 1234)
  const configPath = join(dir, 'draft-review.json')
  const fileOf = (json: unknown) => {
    const path = join(dir, `input-${crypto.randomUUID()}.json`)
    writeFileSync(path, typeof json === 'string' ? json : JSON.stringify(json))
    return path
  }
  const start = (args: string[]) =>
    Bun.spawn(['bun', MAIN, ...args, '--store', root, '--repo', 'acme/widgets'], {stdout: 'pipe', stderr: 'pipe', env})
  const finish = async (proc: ReturnType<typeof start>): Promise<Run> => ({
    code: await proc.exited,
    stdout: await new Response(proc.stdout).text(),
    stderr: await new Response(proc.stderr).text()
  })
  const evidence = (name = 'blank.png', bytes = 'png bytes') => {
    const path = join(dir, name)
    writeFileSync(path, bytes)
    return path
  }
  const devQaCiting = (path: string, overrides: Partial<DevQaContribution> = {}) =>
    devQa({body: `Step 2 shows a blank page.\n\n${path}`, findings: [blankPage(path)], uploads: [path], ...overrides})
  return {
    dir,
    store,
    evidence,
    devQaCiting,
    devQa: (overrides: Partial<DevQaContribution> = {}) => devQaCiting(evidence(), overrides),
    configPath,
    file: fileOf,
    start,
    finish,
    cli: (args: string[]) => finish(start(args)),
    add: (json: unknown) => finish(start(['add', fileOf(json)])),
    stored: (head = HEAD) => (existsSync(join(store.prDir, head)) ? readdirSync(join(store.prDir, head)).filter(name => name.endsWith('.json')).sort() : []),
    storedJson: (name: string, head = HEAD): unknown => JSON.parse(readFileSync(join(store.prDir, head, name), 'utf8')),
    writeConfig: (json: unknown) => writeFileSync(configPath, JSON.stringify(json)),
    plant: (relativePath: string, text = 'kept') => {
      const path = join(store.prDir, relativePath)
      mkdirSync(join(path, '..'), {recursive: true})
      writeFileSync(path, text)
      return path
    },
    listing: (head = HEAD) => (existsSync(join(store.prDir, head)) ? readdirSync(join(store.prDir, head)).sort() : []),
    clear: (head = HEAD) => finish(start(['clear', '--pr', '1234', '--head', head])),
    target: ['--pr', '1234', '--head', HEAD]
  }
}

describe('add', () => {
  test('a second contribution of one kind replaces the first, so one file remains', async () => {
    const world = makeWorld()
    expect((await world.add(review({body: 'First.'}))).code).toBe(EXIT.ok)
    expect((await world.add(review({body: 'Second.'}))).code).toBe(EXIT.ok)
    expect(world.stored()).toEqual(['code-review.json'])
    expect(world.storedJson('code-review.json')).toMatchObject({body: 'Second.'})
  })

  test('names a file by kind and platform, so two platforms stay apart', async () => {
    const world = makeWorld()
    await world.add(world.devQa({platform: 'macOS'}))
    await world.add(world.devQa({platform: 'Windows 11/ARM'}))
    expect(world.stored()).toEqual(['dev-qa@Windows%2011%2FARM.json', 'dev-qa@macOS.json'])
  })

  test('two add calls from two processes at once both land', async () => {
    const world = makeWorld()
    const runs = await Promise.all([world.add(review()), world.add(world.devQa())])
    expect(runs.map(run => run.code)).toEqual([EXIT.ok, EXIT.ok])
    expect(world.stored()).toEqual(['code-review.json', 'dev-qa.json'])
  })

  test('lands while another process holds the per-PR lock, because add takes no lock', async () => {
    const world = makeWorld()
    mkdirSync(world.store.prDir, {recursive: true})
    const run = await withLock(world.store.lockPath, 1000, () => world.add(review()))
    expect(run.code).toBe(EXIT.ok)
    expect(world.stored()).toEqual(['code-review.json'])
  })

  test('refuses a contribution that cites a missing file with exit 66, naming it, and stores nothing', async () => {
    const world = makeWorld()
    const missing = join(world.dir, 'missing.png')
    const run = await world.add(world.devQaCiting(missing))
    expect(run.code).toBe(EXIT.noInput)
    expect(run.stderr).toContain(missing)
    expect(world.stored()).toEqual([])
  })

  test('refuses an invalid contribution with exit 65, naming the field path, and stores nothing', async () => {
    const world = makeWorld()
    const run = await world.add({...world.devQa(), uploads: []})
    expect(run.code).toBe(EXIT.dataErr)
    expect(run.stderr).toContain('findings.0.evidence')
    expect(world.stored()).toEqual([])
  })

  test('refuses text that is not JSON with exit 65', async () => {
    const run = await makeWorld().add('{')
    expect(run.code).toBe(EXIT.dataErr)
    expect(run.stderr).toContain('not JSON')
  })

  test('refuses a missing file with exit 66, naming it', async () => {
    const world = makeWorld()
    const run = await world.cli(['add', join(world.dir, 'absent.json')])
    expect(run.code).toBe(EXIT.noInput)
    expect(run.stderr).toContain('absent.json')
  })
})

describe('add removes the round-two files that answered the kind it replaces', () => {
  const reproduced = (path: string) => verify([{id: 'R1', result: 'reproduced', evidence: [path], note: 'Seen on the build.'}])
  const rootCause = trace([{id: 'Q1', result: 'root-cause', anchor: {path: 'src/b.ts', line: 3, side: 'RIGHT'}, body: 'The cause.'}])

  test('a code-review removes every dev-qa-verify file and names each removed file on stderr', async () => {
    const world = makeWorld()
    await world.add(review())
    await world.add(reproduced(world.evidence('r1.mp4')))
    const run = await world.add(review({body: 'Second run.'}))
    expect(run.code).toBe(EXIT.ok)
    expect(world.listing()).toEqual(['code-review.json', 'code-review.json.files'])
    expect(run.stderr).toContain('dev-qa-verify.json')
    expect(run.stderr).toContain('dev-qa-verify.json.files')
  })

  test('a dev-qa removes the code-trace file and keeps the dev-qa-verify file', async () => {
    const world = makeWorld()
    await world.add(review())
    await world.add(world.devQa())
    await world.add(reproduced(world.evidence('r1.mp4')))
    await world.add(rootCause)
    const run = await world.add(world.devQa({body: 'Second run.'}))
    expect(run.code).toBe(EXIT.ok)
    expect(world.stored()).toEqual(['code-review.json', 'dev-qa-verify.json', 'dev-qa.json'])
    expect(run.stderr).toContain('code-trace.json')
  })

  test('a round-two contribution removes nothing', async () => {
    const world = makeWorld()
    await world.add(review())
    await world.add(world.devQa())
    await world.add(rootCause)
    const run = await world.add(reproduced(world.evidence('r1.mp4')))
    expect(run.code).toBe(EXIT.ok)
    expect(world.stored()).toEqual(['code-review.json', 'code-trace.json', 'dev-qa-verify.json', 'dev-qa.json'])
    expect(run.stderr).toBe('')
  })
})

describe('add checks the merge rules before it stores', () => {
  test('refuses with exit 65 when the stored set plus the new file breaks a merge rule, names the rule, and stores nothing', async () => {
    const world = makeWorld()
    await world.add(review())
    const before = world.listing()
    const run = await world.add(verify([{id: 'R2', result: 'reproduced', evidence: [world.evidence('r2.mp4')], note: 'Seen.'}]))
    expect(run.code).toBe(EXIT.dataErr)
    expect(run.stderr).toContain('R2, which is not a review finding with a behavior')
    expect(world.listing()).toEqual(before)
  })

  test('a refused round-one add removes no round-two file', async () => {
    const world = makeWorld()
    await world.add(world.devQa({platform: 'macOS'}))
    await world.add(trace([{id: 'Q1@macOS', result: 'no-code-cause', body: 'The build config.'}]))
    const before = world.listing()
    const run = await world.add(world.devQa())
    expect(run.code).toBe(EXIT.dataErr)
    expect(world.listing()).toEqual(before)
    expect(before).toContain('code-trace.json')
  })
})

describe('route', () => {
  test('writes the findings for each other skill as {"findings": [...]}', async () => {
    const world = makeWorld()
    const png = world.evidence()
    await world.add(review())
    await world.add(world.devQaCiting(png))
    const out = join(world.dir, 'routed')
    expect((await world.cli(['route', ...world.target, '--out', out])).code).toBe(EXIT.ok)
    const read = (name: string) => JSON.parse(readFileSync(join(out, name), 'utf8'))
    expect(read('to-verify.json')).toEqual({findings: [review().findings[0]]})
    expect(read('to-trace.json')).toEqual({findings: [blankPage(png)]})
  })

  test('refuses a stored set that breaks a merge rule with exit 65 and the reason', async () => {
    const world = makeWorld()
    await world.add(world.devQa({platform: 'macOS'}))
    world.plant(`${HEAD}/dev-qa.json`, JSON.stringify(world.devQa()))
    const run = await world.cli(['route', ...world.target, '--out', join(world.dir, 'routed')])
    expect(run.code).toBe(EXIT.dataErr)
    expect(run.stderr).toContain('platform')
  })

  test('refuses a head with nothing stored with exit 66', async () => {
    const world = makeWorld()
    const run = await world.cli(['route', ...world.target, '--out', join(world.dir, 'routed')])
    expect(run.code).toBe(EXIT.noInput)
    expect(run.stderr).toContain(HEAD)
  })
})

describe('show', () => {
  test('prints the body with its attribution, each inline comment, and the local paths', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {attribution: 'Posted on behalf of @someone'}}})
    const png = world.evidence()
    await world.add(review())
    await world.add(world.devQaCiting(png))
    const run = await world.cli(['show', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.ok)
    expect(run.stdout).toContain('Posted on behalf of @someone\n\nThe refresh path has one defect.')
    expect(run.stdout).toContain('--- inline comment: src/a.ts line 42 (RIGHT) ---')
    expect(run.stdout).toContain(`--- files to upload ---\n\n${png}`)
  })

  test('marks a local path that is not in uploads, with its location', async () => {
    const world = makeWorld()
    await world.add(review({body: 'Notes are in /Users/someone/notes.md.'}))
    const run = await world.cli(['show', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.ok)
    expect(run.stdout).toContain('--- local paths that create refuses')
    expect(run.stdout).toContain('the body: /Users/someone/notes.md')
  })
})

describe('create', () => {
  test('with no config file, exits 78 and names the file', async () => {
    const world = makeWorld()
    await world.add(review())
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.config)
    expect(run.stderr).toContain(world.configPath)
  })

  test('with no entry for the repository, exits 78 and names the repository', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/other': {}}})
    await world.add(review())
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.config)
    expect(run.stderr).toContain('acme/widgets')
  })

  test('with an invalid config file, exits 78 and names the field', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {attribution: 7}}})
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.config)
    expect(run.stderr).toContain('attribution')
  })

  test('when a cited file needs an upload and no uploadEndpoint is set, exits 78 naming the file and the key', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {}}})
    const png = world.evidence()
    await world.add(world.devQaCiting(png))
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.config)
    expect(run.stderr).toContain(png)
    expect(run.stderr).toContain('uploadEndpoint')
  })

  test('when a cited file is gone and was never uploaded, exits 66 naming the file', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {}}})
    const png = world.evidence('gone.png')
    await world.add(world.devQaCiting(png))
    rmSync(png)
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.noInput)
    expect(run.stderr).toContain(png)
  })

  test('refuses with exit 65 when the body holds a local path that is not in uploads, and names the path and its location', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {}}})
    await world.add(review({body: 'Notes are in /Users/someone/notes.md.'}))
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.dataErr)
    expect(run.stderr).toContain('the body holds the local path /Users/someone/notes.md')
    expect(run.stderr).not.toContain(FAKE_GH_MESSAGE)
  })

  test('a run that reaches gh gets the fake gh, so no test calls GitHub', async () => {
    const world = makeWorld()
    world.writeConfig({repositories: {'acme/widgets': {}}})
    await world.add(review())
    const run = await world.cli(['create', ...world.target, '--config', world.configPath])
    expect(run.code).toBe(EXIT.unavailable)
    expect(run.stderr).toContain(FAKE_GH_MESSAGE)
  })
})

describe('clear', () => {
  test('removes the contributions and hash records of the head, prints their names, and keeps every other file', async () => {
    const world = makeWorld()
    await world.add(review())
    await world.add(world.devQa({platform: 'macOS'}))
    await world.add(review({head: OTHER_HEAD}))
    world.plant(`${HEAD}/synthesized.md`)
    world.plant(`${HEAD}/payload.request`)
    world.plant(`${HEAD}/notes.json`)
    world.plant('uploads.json', '{"entries": []}')
    world.plant('draft.json', '{}')
    const run = await world.clear()
    expect(run.code).toBe(EXIT.ok)
    expect(run.stdout.trim().split('\n')).toEqual(['code-review.json', 'code-review.json.files', 'dev-qa@macOS.json', 'dev-qa@macOS.json.files'])
    expect(world.listing()).toEqual(['notes.json', 'payload.request', 'synthesized.md'])
    expect(world.stored(OTHER_HEAD)).toEqual(['code-review.json'])
    expect(existsSync(join(world.store.prDir, 'uploads.json'))).toBe(true)
    expect(existsSync(join(world.store.prDir, 'draft.json'))).toBe(true)
  })

  test('removes a hash record whose contribution is gone', async () => {
    const world = makeWorld()
    world.plant(`${HEAD}/code-trace.json.files`, '{"files": []}')
    const run = await world.clear()
    expect(run.code).toBe(EXIT.ok)
    expect(run.stdout).toBe('code-trace.json.files\n')
    expect(world.listing()).toEqual([])
  })

  test('with no folder for the head, prints nothing and exits 0', async () => {
    const world = makeWorld()
    const run = await world.clear()
    expect(run.code).toBe(EXIT.ok)
    expect(run.stdout).toBe('')
  })

  test('waits for the per-PR lock before it removes a file', async () => {
    const world = makeWorld()
    await world.add(review())
    const whileHeld = await withLock(world.store.lockPath, 1000, async () => {
      const proc = world.start(['clear', ...world.target])
      await Bun.sleep(500)
      return {proc, listing: world.listing()}
    })
    const run = await world.finish(whileHeld.proc)
    expect(whileHeld.listing).toEqual(['code-review.json', 'code-review.json.files'])
    expect(run.code).toBe(EXIT.ok)
    expect(world.listing()).toEqual([])
  })
})

describe('draftAction', () => {
  const comment = (id: number, body: string) => ({id, body})
  const record = (body: string, comments: {id: number; body: string}[] = [], reviewId = 7, head = HEAD): DraftRecord => ({
    reviewId,
    bodyHash: bodyHash(body),
    comments: comments.map(item => ({id: item.id, bodyHash: bodyHash(item.body)})),
    contributionsDigest: 'd'.repeat(64),
    head,
    url: 'https://example.test/review'
  })
  const pending = (body: string, comments: {id: number; body: string}[] = [], id = 7) => ({id, body, comments})
  const written = [comment(11, 'First.'), comment(12, 'Second.')]
  const NO_RECORD: RecordedReview = {kind: 'none'}
  const inState = (lastWritten: DraftRecord, state: string): RecordedReview => ({kind: 'found', record: lastWritten, state})
  const stillPending = (lastWritten: DraftRecord) => inState(lastWritten, 'PENDING')
  const decide = (existing: PendingReview[], recorded: RecordedReview, submitted: SubmittedReview[] = [], body = 'Merged body.') =>
    draftAction({pending: existing, submitted, recorded, head: HEAD, body})

  test('with no pending review, creates one', () => {
    expect(decide([], NO_RECORD)).toEqual({kind: 'create'})
  })

  test('replaces the review it created when the body and every comment still match', () => {
    expect(decide([pending('Body.')], stillPending(record('Body.')))).toEqual({kind: 'replace', reviewId: 7})
    expect(decide([pending('Body.', [...written].reverse())], stillPending(record('Body.', written)))).toEqual({kind: 'replace', reviewId: 7})
  })

  test('refuses when the user edited the body of that review', () => {
    expect(decide([pending('Body, edited by hand.')], stillPending(record('Body.')))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user added an inline comment to that review', () => {
    expect(decide([pending('Body.', [...written, comment(13, 'Mine.')])], stillPending(record('Body.', written)))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user edited an inline comment of that review', () => {
    const edited = [written[0] ?? comment(0, ''), comment(12, 'Second, edited.')]
    expect(decide([pending('Body.', edited)], stillPending(record('Body.', written)))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user deleted an inline comment of that review', () => {
    expect(decide([pending('Body.', written.slice(0, 1))], stillPending(record('Body.', written)))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses a pending review that it did not create', () => {
    expect(decide([pending('Body.', [], 9)], stillPending(record('Body.')))).toEqual({kind: 'refuse', reviewId: 9})
    expect(decide([pending('Body.', [], 9)], NO_RECORD)).toEqual({kind: 'refuse', reviewId: 9})
  })

  test.each(['COMMENTED', 'APPROVED', 'CHANGES_REQUESTED', 'DISMISSED'])('refuses, and never posts again, when the user submitted the first draft for this head (state %s)', state => {
    expect(decide([], inState(record('Body.'), state))).toEqual({kind: 'submitted', reviewId: 7, state})
  })

  test('reports the submitted first draft even when another pending review exists', () => {
    expect(decide([pending('Body.', [], 9)], inState(record('Body.'), 'COMMENTED'))).toEqual({kind: 'submitted', reviewId: 7, state: 'COMMENTED'})
  })

  test('treats a state it does not know as submitted, so a new GitHub state cannot cause a second post', () => {
    expect(decide([], inState(record('Body.'), 'SOME_FUTURE_STATE'))).toEqual({kind: 'submitted', reviewId: 7, state: 'SOME_FUTURE_STATE'})
  })

  test('creates a draft when the submitted review belongs to an earlier head', () => {
    expect(decide([], inState(record('Body.', [], 7, OTHER_HEAD), 'COMMENTED'))).toEqual({kind: 'create'})
  })

  test('creates a draft when the user deleted the recorded review', () => {
    expect(decide([], {kind: 'gone', record: record('Body.')})).toEqual({kind: 'create'})
  })

  test('refuses another pending review when the user deleted the recorded review', () => {
    expect(decide([pending('Body.', [], 9)], {kind: 'gone', record: record('Body.')})).toEqual({kind: 'refuse', reviewId: 9})
  })

  const submittedReview = (body: string, commitId: string | undefined = HEAD, state = 'COMMENTED', id = 8): SubmittedReview => ({id, state, commitId, body})

  test('refuses a body that a submitted review for this head already holds, with no draft.json', () => {
    expect(decide([], NO_RECORD, [submittedReview('Merged body.')], 'Merged body.')).toEqual({kind: 'duplicate', reviewId: 8, state: 'COMMENTED'})
  })

  test('compares the bodies after CRLF becomes LF and trailing whitespace goes', () => {
    expect(decide([], NO_RECORD, [submittedReview('Line one.\r\n\r\nLine two.  \r\n')], 'Line one.\n\nLine two.')).toEqual({kind: 'duplicate', reviewId: 8, state: 'COMMENTED'})
  })

  test('creates a draft when the submitted review with the same body belongs to another commit', () => {
    expect(decide([], NO_RECORD, [submittedReview('Merged body.', OTHER_HEAD)], 'Merged body.')).toEqual({kind: 'create'})
  })

  test('creates a draft when the submitted review for this head holds another body', () => {
    expect(decide([], NO_RECORD, [submittedReview('Older findings.')], 'Merged body.')).toEqual({kind: 'create'})
  })

  test('ignores a pending review in the submitted list', () => {
    expect(decide([], NO_RECORD, [submittedReview('Merged body.', HEAD, 'PENDING')], 'Merged body.')).toEqual({kind: 'create'})
  })
})

describe('the second create after the user submitted a draft that draft.json does not name', () => {
  const BODY = 'Merged body.'
  const submitted: SubmittedReview[] = [{id: 7, state: 'COMMENTED', commitId: HEAD, body: BODY}]
  const decideWith = (recorded: RecordedReview) => draftAction({pending: [], submitted, recorded, head: HEAD, body: BODY})
  const staleRecord: DraftRecord = {reviewId: 5, bodyHash: bodyHash(BODY), comments: [], head: HEAD, url: 'https://example.test/review/5'}

  test('refuses after a read-back failure left no draft.json', () => {
    const afterCreate = draftRecordAfterCreate({review: {id: 7, body: BODY, url: 'https://example.test/review/7'}, head: HEAD, requestedBodies: [], readBack: {kind: 'failed', reason: 'HTTP 502'}, contributionsDigest: 'd'.repeat(64)})
    expect(afterCreate.kind).toBe('skip')
    expect(decideWith({kind: 'none'})).toEqual({kind: 'duplicate', reviewId: 7, state: 'COMMENTED'})
  })

  test('refuses when a stale draft.json names a review that GitHub no longer has (404)', () => {
    expect(decideWith({kind: 'gone', record: staleRecord})).toEqual({kind: 'duplicate', reviewId: 7, state: 'COMMENTED'})
  })

  test('refuses after the user deleted draft.json by hand', () => {
    const world = makeWorld()
    writeDraftRecord(world.store, staleRecord)
    rmSync(join(world.store.prDir, 'draft.json'))
    const recorded: RecordedReview = (() => {
      const record = readDraftRecord(world.store)
      return record === undefined ? {kind: 'none'} : {kind: 'gone', record}
    })()
    expect(recorded).toEqual({kind: 'none'})
    expect(decideWith(recorded)).toEqual({kind: 'duplicate', reviewId: 7, state: 'COMMENTED'})
  })
})

describe('readDraftRecord', () => {
  const recordFields = {reviewId: 41, bodyHash: 'a'.repeat(64), comments: [], head: HEAD, url: 'https://example.test/review'}
  const draftWorld = (text: string) => {
    const world = makeWorld()
    mkdirSync(world.store.prDir, {recursive: true})
    writeFileSync(join(world.store.prDir, 'draft.json'), text)
    return world
  }
  const attempt = (store: PrStore) => {
    try {
      return {record: readDraftRecord(store)}
    } catch (error) {
      return error instanceof DraftReviewError ? {code: error.exitCode, message: error.message} : {thrown: String(error)}
    }
  }

  test('loads a draft.json from an older round that has no contributions digest', () => {
    expect(attempt(draftWorld(JSON.stringify(recordFields)).store)).toEqual({record: recordFields})
  })

  test('refuses an invalid draft.json with exit 65, names its pending review, and says what a delete of draft.json does', () => {
    const outcome = attempt(draftWorld(JSON.stringify({...recordFields, bodyHash: 'not a hash'})).store)
    expect(outcome).toMatchObject({code: EXIT.dataErr})
    expect('message' in outcome ? outcome.message : '').toContain(
      'It names review 41. A delete of draft.json is safe: create never posts the content of a submitted review again. While review 41 is pending, the next create refuses to replace it. Delete review 41 on GitHub to unblock create.'
    )
  })

  test('refuses a draft.json that is not JSON with exit 65, and says what a delete of draft.json does', () => {
    const outcome = attempt(draftWorld('{"reviewId": 41,').store)
    expect(outcome).toMatchObject({code: EXIT.dataErr})
    expect('message' in outcome ? outcome.message : '').toContain(
      'A delete of draft.json is safe: create never posts the content of a submitted review again. While the review that draft-review created is pending, the next create refuses to replace it. Delete that review on GitHub to unblock create.'
    )
  })
})

describe('readContributions', () => {
  test('reads the contributions after create wrote the review request and the synthesized text beside them', async () => {
    const world = makeWorld()
    await world.add(review())
    writeReviewRequest(world.store, HEAD, {commit_id: HEAD, body: 'Body.', comments: []})
    writeSynthesized(world.store, HEAD, 'Body.\n')
    expect(readContributions(world.store, HEAD).map(contribution => contribution.kind)).toEqual(['code-review'])
    expect((await world.cli(['route', ...world.target, '--out', join(world.dir, 'routed')])).code).toBe(EXIT.ok)
  })

  test('the head digest of the contributions it read stays the same when create writes its own files', async () => {
    const world = makeWorld()
    await world.add(review())
    const {digest} = readHead(world.store, HEAD)
    writeReviewRequest(world.store, HEAD, {commit_id: HEAD, body: 'Body.', comments: []})
    writeSynthesized(world.store, HEAD, 'Body.\n')
    expect(headDigest(world.store, HEAD)).toBe(digest)
  })

  test('the head digest changes when an add lands after the read', async () => {
    const world = makeWorld()
    await world.add(review())
    const {digest} = readHead(world.store, HEAD)
    await world.add(world.devQa())
    expect(headDigest(world.store, HEAD)).not.toBe(digest)
  })

  test('the head digest changes when an add replaces a contribution after the read', async () => {
    const world = makeWorld()
    await world.add(review({body: 'First.'}))
    const {digest} = readHead(world.store, HEAD)
    await world.add(review({body: 'Second.'}))
    expect(headDigest(world.store, HEAD)).not.toBe(digest)
  })

  test('the head digest changes when an add rewrites a contribution with identical bytes but new evidence content', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'the first recording')
    await world.add(world.devQaCiting(png))
    const contributionBefore = readFileSync(join(world.store.prDir, HEAD, 'dev-qa.json'), 'utf8')
    const {digest} = readHead(world.store, HEAD)
    world.evidence('step.png', 'the second recording')
    await world.add(world.devQaCiting(png))
    expect(readFileSync(join(world.store.prDir, HEAD, 'dev-qa.json'), 'utf8')).toBe(contributionBefore)
    expect(headDigest(world.store, HEAD)).not.toBe(digest)
  })

  test('ignores a JSON file whose name is not a contribution file name', async () => {
    const world = makeWorld()
    await world.add(review())
    writeFileSync(join(world.store.prDir, HEAD, 'payload.json'), JSON.stringify({commit_id: HEAD, body: 'Body.', comments: []}))
    writeFileSync(join(world.store.prDir, HEAD, 'notes.json'), '{}')
    expect(readContributions(world.store, HEAD).map(contribution => contribution.kind)).toEqual(['code-review'])
  })
})

describe('prepareUploads and runUploads', () => {
  const resolveUploads = async (store: PrStore, head: string, paths: string[], uploaderFor: () => IUploader) =>
    runUploads(store, prepareUploads(store, head, paths, uploaderFor))

  function uploadWorld() {
    const world = makeWorld()
    mkdirSync(world.store.prDir, {recursive: true})
    const first = join(world.dir, 'first.png')
    const second = join(world.dir, 'second.png')
    writeFileSync(first, 'bytes of the first file')
    writeFileSync(second, 'bytes of the second file')
    return {world, first, second, paths: [first, second]}
  }

  const uploaderThat = (name: string, failOn: number | undefined) => {
    const uploaded: string[] = []
    const uploader: IUploader = {
      upload: async ({path}) => {
        if (uploaded.length + 1 === failOn) throw new Error(`upload of ${path} failed`)
        uploaded.push(path)
        return `https://cdn.example/${name}/${uploaded.length}`
      }
    }
    return {uploader, uploaded}
  }

  test('keeps the URLs that a failed run uploaded, and the next run uploads only the rest', async () => {
    const {world, first, second, paths} = uploadWorld()
    const failing = uploaderThat('failing', 2)
    await expect(resolveUploads(world.store, HEAD, paths, () => failing.uploader)).rejects.toThrow('second.png')
    const working = uploaderThat('working', undefined)
    const urls = await resolveUploads(world.store, HEAD, paths, () => working.uploader)
    expect(failing.uploaded).toEqual([first])
    expect(working.uploaded).toEqual([second])
    expect(urls).toEqual(new Map([[first, 'https://cdn.example/failing/1'], [second, 'https://cdn.example/working/1']]))
  })

  const memoryCdn = () => {
    const contentByUrl = new Map<string, string>()
    const uploader: IUploader = {
      upload: async ({bytes}) => {
        const url = `https://cdn.example/memory/${contentByUrl.size + 1}`
        contentByUrl.set(url, bodyHash(new TextDecoder().decode(bytes)))
        return url
      }
    }
    return {uploader, contentByUrl}
  }

  const ledgerEntries = (store: PrStore): {sha256: string; url: string}[] => JSON.parse(readFileSync(join(store.prDir, 'uploads.json'), 'utf8')).entries

  test('records each upload under the hash of the bytes it sent, when a file changes between prepare and upload', async () => {
    const {world, second, paths} = uploadWorld()
    const cdn = memoryCdn()
    const prepared = prepareUploads(world.store, HEAD, paths, () => cdn.uploader)
    writeFileSync(second, 'bytes written after prepare')
    const outcome = await refusal(runUploads(world.store, prepared))
    expect(ledgerEntries(world.store).map(entry => ({sha256: entry.sha256, sent: cdn.contentByUrl.get(entry.url)}))).toEqual([
      {sha256: bodyHash('bytes of the first file'), sent: bodyHash('bytes of the first file')}
    ])
    expect(outcome).toEqual({code: EXIT.noInput, message: `${second} changed while create ran. Run create again.`})
    expect(cdn.contentByUrl.size).toBe(1)
  })

  test('two prepares of the same store give the same body, so a later create can match a submitted body', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'the recording')
    await world.add(world.devQaCiting(png))
    const bodyFor = (urls: ReadonlyMap<string, string>) => substituteUploads(payloadOf(readContributions(world.store, HEAD)), urls).body
    const first = bodyFor(await resolveUploads(world.store, HEAD, [png], () => memoryCdn().uploader))
    const second = bodyFor(prepareUploads(world.store, HEAD, [png], noUploader).plan.urlByPath)
    expect(second).toBe(first)
    expect(first).toContain('https://cdn.example/memory/1')
    expect(first).not.toContain(png)
  })

  test('asks for no uploader when every file is in the ledger', async () => {
    const {world, paths} = uploadWorld()
    await resolveUploads(world.store, HEAD, paths, () => uploaderThat('first', undefined).uploader)
    const urls = await resolveUploads(world.store, HEAD, paths, () => {
      throw new Error('no uploader expected')
    })
    expect([...urls.keys()]).toEqual(paths)
  })

  const noUploader = (): IUploader => {
    throw new Error('no uploader expected')
  }

  test('resolves a gone file by the content hash that add recorded', async () => {
    const world = makeWorld()
    const png = world.evidence()
    await world.add(world.devQaCiting(png))
    const first = await resolveUploads(world.store, HEAD, [png], () => uploaderThat('first', undefined).uploader)
    rmSync(png)
    expect(await resolveUploads(world.store, HEAD, [png], noUploader)).toEqual(first)
  })

  test('refuses a gone file with exit 66 when its recorded content was never uploaded, even when the path was uploaded for another head', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'the old recording')
    await world.add(world.devQaCiting(png, {head: OTHER_HEAD}))
    await resolveUploads(world.store, OTHER_HEAD, [png], () => uploaderThat('old', undefined).uploader)
    world.evidence('step.png', 'the new recording')
    await world.add(world.devQaCiting(png))
    rmSync(png)
    const caught = await resolveUploads(world.store, HEAD, [png], noUploader).catch((error: unknown) => error)
    expect(caught instanceof DraftReviewError ? caught.exitCode : caught).toBe(EXIT.noInput)
  })

  const refusal = async (promise: Promise<unknown>) => {
    const caught = await promise.catch((error: unknown) => error)
    return caught instanceof DraftReviewError ? {code: caught.exitCode, message: caught.message} : {code: undefined, message: String(caught)}
  }

  test('refuses a file that changed after add with exit 66 and tells the caller to run add again', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'what the author saw')
    await world.add(world.devQaCiting(png))
    world.evidence('step.png', 'a later recording')
    expect(await refusal(resolveUploads(world.store, HEAD, [png], noUploader))).toEqual({code: EXIT.noInput, message: `${png} changed after add, run add again`})
  })

  test('uses the upload of the content that add recorded when the file changed after that upload', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'what the author saw')
    await world.add(world.devQaCiting(png))
    const first = await resolveUploads(world.store, HEAD, [png], () => uploaderThat('first', undefined).uploader)
    world.evidence('step.png', 'a later recording')
    expect(await resolveUploads(world.store, HEAD, [png], noUploader)).toEqual(first)
  })

  const settled = (promise: Promise<Map<string, string>>) => promise.then(urls => ({urls}), (error: unknown) => ({error: String(error)}))
  const orphanRecord = (world: ReturnType<typeof makeWorld>, name: string, path: string) =>
    writeFileSync(join(world.store.prDir, HEAD, name), JSON.stringify({files: [{path, sha256: 'b'.repeat(64)}]}))

  test('ignores the hash record of an add that crashed before it wrote its contribution', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'what the author saw')
    await world.add(world.devQaCiting(png))
    orphanRecord(world, 'dev-qa@Windows.json.files', png)
    expect(await settled(resolveUploads(world.store, HEAD, [png], () => uploaderThat('first', undefined).uploader))).toEqual({urls: new Map([[png, 'https://cdn.example/first/1']])})
  })

  test('ignores a hash record whose name is not a contribution file name', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'what the author saw')
    await world.add(world.devQaCiting(png))
    orphanRecord(world, 'notes.json.files', png)
    expect(await settled(resolveUploads(world.store, HEAD, [png], () => uploaderThat('first', undefined).uploader))).toEqual({urls: new Map([[png, 'https://cdn.example/first/1']])})
  })

  test('refuses with exit 66 when two contributions for one head recorded different content for one path', async () => {
    const world = makeWorld()
    const png = world.evidence('step.png', 'the first recording')
    await world.add(world.devQaCiting(png, {platform: 'macOS'}))
    world.evidence('step.png', 'the second recording')
    await world.add(world.devQaCiting(png, {platform: 'Windows'}))
    const result = await refusal(resolveUploads(world.store, HEAD, [png], noUploader))
    expect(result.code).toBe(EXIT.noInput)
    expect(result.message).toContain(png)
    expect(result.message).toContain('different content')
  })
})
