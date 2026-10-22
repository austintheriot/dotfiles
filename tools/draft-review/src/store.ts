import {createHash} from 'node:crypto'
import {existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync} from 'node:fs'
import {basename, dirname, join} from 'node:path'
import {z} from 'zod'
import type {Repo} from './args'
import {planAdd} from './add-plan'
import {CONTRIBUTION_FILE_NAME, FILE_HASHES_SUFFIX, namesToClear} from './clear'
import type {Contribution} from './contribution'
import {parseContribution} from './contribution'
import type {RoutedFindings} from './contribution-set'
import type {DraftRecord, EarlierDraft} from './draft'
import {bodyHash, contributionsDigest, parseDraftRecord} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {CitedFile, Ledger, RecordedHash, UnresolvedFile, UploadPlan} from './ledger'
import {EMPTY_LEDGER, LedgerSchema, planUploads, recordUpload, Sha256Schema} from './ledger'

export type UploadFile = {path: string; bytes: Uint8Array}

export interface IUploader {
  upload(file: UploadFile): Promise<string>
}

export type PrStore = {prDir: string; lockPath: string}

const LEDGER_FILE = 'uploads.json'
const DRAFT_FILE = 'draft.json'
const SYNTHESIZED_FILE = 'synthesized.md'
const REQUEST_FILE = 'payload.request'

const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

export function prStore(root: string, repo: Repo, pr: number): PrStore {
  const prDir = join(root, repo.owner, repo.name, String(pr))
  return {prDir, lockPath: join(prDir, '.lock')}
}

const headDir = (store: PrStore, head: string) => join(store.prDir, head)

function writeFileAtomic(path: string, text: string): void {
  const temporary = join(dirname(path), `.${basename(path)}.${process.pid}.${crypto.randomUUID()}.tmp`)
  try {
    mkdirSync(dirname(path), {recursive: true})
    writeFileSync(temporary, text)
    renameSync(temporary, path)
  } catch (error) {
    rmSync(temporary, {force: true})
    throw new DraftReviewError(`cannot write ${path}: ${message(error)}`, EXIT.ioErr)
  }
}

function readText(path: string): string {
  try {
    return readFileSync(path, 'utf8')
  } catch (error) {
    throw new DraftReviewError(`cannot read ${path}: ${message(error)}`, EXIT.ioErr)
  }
}

const writeJson = (path: string, value: unknown) => writeFileAtomic(path, `${JSON.stringify(value, null, 2)}\n`)

function parseStoredContribution(path: string, text: string): Contribution {
  const parsed = parseContribution(text)
  if (!parsed.ok) throw new DraftReviewError(`${path}: ${parsed.reason}`, EXIT.dataErr)
  return parsed.value
}

export function readContributionFile(path: string): Contribution {
  if (!existsSync(path)) throw new DraftReviewError(`no contribution file at ${path}`, EXIT.noInput)
  return parseStoredContribution(path, readText(path))
}

const FileHashesSchema = z.object({files: z.array(z.object({path: z.string().min(1), sha256: Sha256Schema}).strict())}).strict()
type FileHashes = z.infer<typeof FileHashesSchema>

const sha256Of = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex')

const hashFile = (path: string) => sha256Of(readFileSync(path))

function citedFileHashes(contribution: Contribution): FileHashes {
  const missing = contribution.uploads.filter(path => !existsSync(path))
  if (missing.length > 0) throw new DraftReviewError(`the contribution cites files that do not exist: ${missing.join(', ')}`, EXIT.noInput)
  return {files: contribution.uploads.map(path => ({path, sha256: hashFile(path)}))}
}

function removeNames(dir: string, names: readonly string[]): void {
  names.forEach(name => {
    try {
      rmSync(join(dir, name), {force: true})
    } catch (error) {
      throw new DraftReviewError(`cannot remove ${join(dir, name)}: ${message(error)}`, EXIT.ioErr)
    }
  })
}

const listingOf = (dir: string) => (existsSync(dir) ? readdirSync(dir) : [])

export type StoredContribution = {path: string; removed: string[]}

const storedFiles = (dir: string, listing: readonly string[]) =>
  listing
    .filter(name => CONTRIBUTION_FILE_NAME.test(name))
    .sort()
    .map(name => ({name, contribution: parseStoredContribution(join(dir, name), readText(join(dir, name)))}))

// The hashes sit beside the contribution, not inside it, so the stored contribution stays the exact file that add received.
export function writeContribution(store: PrStore, contribution: Contribution): StoredContribution {
  const hashes = citedFileHashes(contribution)
  const dir = headDir(store, contribution.head)
  const listing = listingOf(dir)
  const plan = planAdd({contribution, listing, stored: storedFiles(dir, listing)})
  if (plan.kind === 'refuse') {
    throw new DraftReviewError(`add stored nothing: this contribution and the stored contributions for head ${contribution.head} break a merge rule: ${plan.reason}`, EXIT.dataErr)
  }
  removeNames(dir, plan.remove)
  const path = join(dir, plan.name)
  writeJson(`${path}${FILE_HASHES_SUFFIX}`, hashes)
  writeJson(path, contribution)
  return {path, removed: plan.remove.map(name => join(dir, name))}
}

export function clearHead(store: PrStore, head: string): string[] {
  const dir = headDir(store, head)
  const names = namesToClear(listingOf(dir))
  removeNames(dir, names)
  return names
}

const contributionNames = (dir: string) => readdirSync(dir).filter(name => CONTRIBUTION_FILE_NAME.test(name)).sort()

// Only a record beside a stored contribution counts: an add that crashed between its two writes leaves a record that no contribution cites.
function recordedHashes(store: PrStore, head: string): Map<string, RecordedHash> {
  const dir = headDir(store, head)
  const records = existsSync(dir) ? contributionNames(dir).map(name => join(dir, `${name}${FILE_HASHES_SUFFIX}`)) : []
  const files = records.flatMap(path => readStoredJson(path, FileHashesSchema)?.files ?? [])
  const recordedFor = (path: string): RecordedHash => {
    const hashes = [...new Set(files.filter(file => file.path === path).map(file => file.sha256))].sort()
    const [only] = hashes
    return hashes.length === 1 && only !== undefined ? {kind: 'recorded', sha256: only} : {kind: 'conflict', hashes}
  }
  return new Map([...new Set(files.map(file => file.path))].map(path => [path, recordedFor(path)]))
}

type StoredFile = {name: string; path: string; text: string; hashRecord: string | undefined}

function readContributionFiles(store: PrStore, head: string): StoredFile[] {
  const dir = headDir(store, head)
  if (!existsSync(dir)) throw new DraftReviewError(`no contributions stored for head ${head} in ${dir}`, EXIT.noInput)
  return contributionNames(dir).map(name => {
    const path = join(dir, name)
    const hashRecordPath = `${path}${FILE_HASHES_SUFFIX}`
    return {name, path, text: readText(path), hashRecord: existsSync(hashRecordPath) ? readText(hashRecordPath) : undefined}
  })
}

const digestOf = (files: readonly StoredFile[]) =>
  contributionsDigest(
    files.flatMap(file => [
      {name: file.name, sha256: bodyHash(file.text)},
      ...(file.hashRecord === undefined ? [] : [{name: `${file.name}${FILE_HASHES_SUFFIX}`, sha256: bodyHash(file.hashRecord)}])
    ])
  )

export function readHead(store: PrStore, head: string): {contributions: Contribution[]; digest: string} {
  const files = readContributionFiles(store, head)
  return {contributions: files.map(file => parseStoredContribution(file.path, file.text)), digest: digestOf(files)}
}

export const readContributions = (store: PrStore, head: string): Contribution[] => readHead(store, head).contributions

export const headDigest = (store: PrStore, head: string): string => digestOf(readContributionFiles(store, head))

function parseJsonText(path: string, text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    throw new DraftReviewError(`${path} is not JSON. Fix or delete it.`, EXIT.dataErr)
  }
}

function readStoredJson<Schema extends z.ZodTypeAny>(path: string, schema: Schema): z.infer<Schema> | undefined {
  if (!existsSync(path)) return undefined
  const parsed = schema.safeParse(parseJsonText(path, readText(path)))
  if (!parsed.success) throw new DraftReviewError(`${path} is invalid. Fix or delete it. ${parsed.error.message}`, EXIT.dataErr)
  return parsed.data
}

const ledgerPath = (store: PrStore) => join(store.prDir, LEDGER_FILE)

const readLedger = (store: PrStore): Ledger => readStoredJson(ledgerPath(store), LedgerSchema) ?? EMPTY_LEDGER

const citedFile = (path: string, recorded: Map<string, RecordedHash>): CitedFile => ({
  path,
  observedSha256: existsSync(path) ? hashFile(path) : undefined,
  recorded: recorded.get(path) ?? {kind: 'none'}
})

function unresolvedMessage(file: UnresolvedFile, head: string): string {
  switch (file.reason) {
    case 'gone':
      return `the review cites ${file.path}, which no longer exists, and the ledger has no upload of the content that add recorded for it`
    case 'changed':
      return `${file.path} changed after add, run add again`
    case 'conflict':
      return `two contributions for head ${head} recorded different content for ${file.path}. Run add again for each contribution that cites it`
    default: {
      const unreachable: never = file.reason
      return unreachable
    }
  }
}

async function uploadOne(uploader: IUploader, file: UploadFile): Promise<string> {
  try {
    return await uploader.upload(file)
  } catch (error) {
    if (error instanceof DraftReviewError) throw error
    throw new DraftReviewError(`uploading ${file.path} failed: ${message(error)}`, EXIT.unavailable)
  }
}

function readBytes(path: string): Uint8Array {
  try {
    return readFileSync(path)
  } catch (error) {
    throw new DraftReviewError(`cannot read ${path}: ${message(error)}`, EXIT.noInput)
  }
}

export type PreparedUploads = {cited: CitedFile[]; plan: UploadPlan; uploader: IUploader | undefined}

export function prepareUploads(store: PrStore, head: string, paths: readonly string[], uploaderFor: (firstPath: string) => IUploader): PreparedUploads {
  const recorded = recordedHashes(store, head)
  const cited = paths.map(path => citedFile(path, recorded))
  const plan = planUploads(cited, readLedger(store))
  const [unresolved] = plan.unresolved
  if (unresolved !== undefined) throw new DraftReviewError(unresolvedMessage(unresolved, head), EXIT.noInput)
  const [first] = plan.toUpload
  return {cited, plan, uploader: first === undefined ? undefined : uploaderFor(first.path)}
}

export async function runUploads(store: PrStore, prepared: PreparedUploads): Promise<Map<string, string>> {
  const {cited, plan, uploader} = prepared
  if (uploader === undefined) return plan.urlByPath
  for (const file of plan.toUpload) {
    const bytes = readBytes(file.path)
    if (sha256Of(bytes) !== file.sha256) throw new DraftReviewError(`${file.path} changed while create ran. Run show, then create, again.`, EXIT.noInput)
    const url = await uploadOne(uploader, {path: file.path, bytes})
    writeJson(ledgerPath(store), recordUpload(readLedger(store), {...file, url}))
  }
  const final = planUploads(cited, readLedger(store))
  if (final.toUpload.length > 0) throw new DraftReviewError(`the ledger ${ledgerPath(store)} did not record every upload`, EXIT.ioErr)
  return final.urlByPath
}

export function readDraftRecord(store: PrStore): DraftRecord | undefined {
  const path = join(store.prDir, DRAFT_FILE)
  if (!existsSync(path)) return undefined
  const parsed = parseDraftRecord(readText(path))
  if (!parsed.ok) throw new DraftReviewError(`${path} is invalid: ${parsed.reason}`, EXIT.dataErr)
  return parsed.value
}

export const synthesizedPathFor = (store: PrStore, head: string): string => join(headDir(store, head), SYNTHESIZED_FILE)

export function earlierDraft(store: PrStore): EarlierDraft | undefined {
  const record = readDraftRecord(store)
  return record === undefined ? undefined : {head: record.head, synthesizedPath: synthesizedPathFor(store, record.head)}
}

export const writeDraftRecord =(store: PrStore, record: DraftRecord): void => writeJson(join(store.prDir, DRAFT_FILE), record)

export function writeSynthesized(store: PrStore, head: string, text: string): string {
  const path = synthesizedPathFor(store, head)
  writeFileAtomic(path, text)
  return path
}

export function writeReviewRequest(store: PrStore, head: string, request: unknown): string {
  const path = join(headDir(store, head), REQUEST_FILE)
  writeJson(path, request)
  return path
}

export function writeRouted(out: string, routed: RoutedFindings): string[] {
  const files: [string, RoutedFindings['toVerify']][] = [
    [join(out, 'to-verify.json'), routed.toVerify],
    [join(out, 'to-trace.json'), routed.toTrace]
  ]
  files.forEach(([path, findings]) => writeJson(path, {findings}))
  return files.map(([path]) => path)
}
