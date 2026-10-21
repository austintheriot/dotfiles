import {afterAll, describe, expect, test} from 'bun:test'
import {existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {bodyHash, draftAction} from './draft'
import type {DraftRecord} from './draft'
import {DraftReviewError, EXIT} from './exit'
import {withLock} from './lock'
import type {IUploader} from './store'
import type {PrStore} from './store'
import {headDigest, prepareUploads, prStore, readContributions, readHead, runUploads, writeReviewRequest, writeSynthesized} from './store'
import type {DevQaContribution} from './contribution'
import {blankPage, devQa, HEAD, OTHER_HEAD, review} from './testing/fixtures'

const MAIN = join(import.meta.dir, 'main.ts')
const REPO = {owner: 'acme', name: 'widgets'}
const worlds: string[] = []

afterAll(() => worlds.forEach(dir => rmSync(dir, {recursive: true, force: true})))

type Run = {code: number; stdout: string; stderr: string}

function makeWorld() {
  const dir = mkdtempSync(join(tmpdir(), 'draft-review-test-'))
  worlds.push(dir)
  const root = join(dir, 'store')
  const store = prStore(root, REPO, 1234)
  const configPath = join(dir, 'draft-review.json')
  const fileOf = (json: unknown) => {
    const path = join(dir, `input-${crypto.randomUUID()}.json`)
    writeFileSync(path, typeof json === 'string' ? json : JSON.stringify(json))
    return path
  }
  const start = (args: string[]) =>
    Bun.spawn(['bun', MAIN, ...args, '--store', root, '--repo', 'acme/widgets'], {stdout: 'pipe', stderr: 'pipe', env: {...process.env, NO_COLOR: '1'}})
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

  test('refuses a set that breaks a merge rule with exit 65 and the reason', async () => {
    const world = makeWorld()
    await world.add(world.devQa({platform: 'macOS'}))
    await world.add(world.devQa())
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
})

describe('draftAction', () => {
  const comment = (id: number, body: string) => ({id, body})
  const record = (body: string, comments: {id: number; body: string}[] = [], reviewId = 7): DraftRecord => ({
    reviewId,
    bodyHash: bodyHash(body),
    comments: comments.map(item => ({id: item.id, bodyHash: bodyHash(item.body)})),
    contributionsDigest: 'd'.repeat(64),
    head: HEAD,
    url: 'https://example.test/review'
  })
  const pending = (body: string, comments: {id: number; body: string}[] = [], id = 7) => ({id, body, comments})
  const written = [comment(11, 'First.'), comment(12, 'Second.')]

  test('with no pending review, creates one', () => {
    expect(draftAction([], record('Body.'))).toEqual({kind: 'create'})
    expect(draftAction([], undefined)).toEqual({kind: 'create'})
  })

  test('replaces the review it created when the body and every comment still match', () => {
    expect(draftAction([pending('Body.')], record('Body.'))).toEqual({kind: 'replace', reviewId: 7})
    expect(draftAction([pending('Body.', [...written].reverse())], record('Body.', written))).toEqual({kind: 'replace', reviewId: 7})
  })

  test('refuses when the user changed the body of that review', () => {
    expect(draftAction([pending('Body, edited by hand.')], record('Body.'))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user added an inline comment to that review', () => {
    expect(draftAction([pending('Body.', [...written, comment(13, 'Mine.')])], record('Body.', written))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user edited an inline comment of that review', () => {
    expect(draftAction([pending('Body.', [written[0] ?? comment(0, ''), comment(12, 'Second, edited.')])], record('Body.', written))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses when the user deleted an inline comment of that review', () => {
    expect(draftAction([pending('Body.', written.slice(0, 1))], record('Body.', written))).toEqual({kind: 'refuse', reviewId: 7})
  })

  test('refuses a pending review that it did not create', () => {
    expect(draftAction([pending('Body.', [], 9)], record('Body.'))).toEqual({kind: 'refuse', reviewId: 9})
    expect(draftAction([pending('Body.', [], 9)], undefined)).toEqual({kind: 'refuse', reviewId: 9})
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
      upload: async path => {
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
