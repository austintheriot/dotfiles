import {createHash} from 'node:crypto'
import {existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync} from 'node:fs'
import {basename, dirname, join} from 'node:path'
import {z} from 'zod'
import type {Repo} from './args'
import type {Contribution} from './contribution'
import {parseContribution} from './contribution'
import type {RoutedFindings} from './contribution-set'
import type {DraftRecord} from './draft'
import {DraftRecordSchema} from './draft'
import {DraftReviewError, EXIT} from './exit'
import type {CitedFile, Ledger, UploadPlan} from './ledger'
import {EMPTY_LEDGER, LedgerSchema, planUploads, recordUpload, Sha256Schema} from './ledger'

export interface IUploader {
  upload(path: string): Promise<string>
}

export type PrStore = {prDir: string; lockPath: string}

const LEDGER_FILE = 'uploads.json'
const DRAFT_FILE = 'draft.json'
const SYNTHESIZED_FILE = 'synthesized.md'
const REQUEST_FILE = 'payload.json'

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

// A platform label is free text but never contains @, so @ keeps `dev-qa` with platform `verify-x` apart from `dev-qa-verify` with platform `x`.
const contributionFileName = (contribution: Contribution) => {
  const platform = contribution.kind === 'dev-qa' || contribution.kind === 'dev-qa-verify' ? contribution.platform : undefined
  return platform === undefined ? `${contribution.kind}.json` : `${contribution.kind}@${encodeURIComponent(platform)}.json`
}

const FileHashesSchema = z.object({files: z.array(z.object({path: z.string().min(1), sha256: Sha256Schema}).strict())}).strict()
type FileHashes = z.infer<typeof FileHashesSchema>

const FILE_HASHES_SUFFIX = '.files'

const hashFile = (path: string) => createHash('sha256').update(readFileSync(path)).digest('hex')

function citedFileHashes(contribution: Contribution): FileHashes {
  const missing = contribution.uploads.filter(path => !existsSync(path))
  if (missing.length > 0) throw new DraftReviewError(`the contribution cites files that do not exist: ${missing.join(', ')}`, EXIT.noInput)
  return {files: contribution.uploads.map(path => ({path, sha256: hashFile(path)}))}
}

// The hashes sit beside the contribution, not inside it, so the stored contribution stays the exact file that add received.
export function writeContribution(store: PrStore, contribution: Contribution): string {
  const hashes = citedFileHashes(contribution)
  const path = join(headDir(store, contribution.head), contributionFileName(contribution))
  writeJson(`${path}${FILE_HASHES_SUFFIX}`, hashes)
  writeJson(path, contribution)
  return path
}

function recordedHashes(store: PrStore, head: string): Map<string, string | undefined> {
  const dir = headDir(store, head)
  const records = existsSync(dir) ? readdirSync(dir).filter(name => name.endsWith(`.json${FILE_HASHES_SUFFIX}`)) : []
  const files = records.flatMap(name => readStoredJson(join(dir, name), FileHashesSchema)?.files ?? [])
  const onlyHash = (path: string) => {
    const hashes = [...new Set(files.filter(file => file.path === path).map(file => file.sha256))]
    return hashes.length === 1 ? hashes[0] : undefined
  }
  return new Map([...new Set(files.map(file => file.path))].map(path => [path, onlyHash(path)]))
}

export function readContributions(store: PrStore, head: string): Contribution[] {
  const dir = headDir(store, head)
  if (!existsSync(dir)) throw new DraftReviewError(`no contributions stored for head ${head} in ${dir}`, EXIT.noInput)
  return readdirSync(dir)
    .filter(name => name.endsWith('.json') && !name.startsWith('.'))
    .sort()
    .map(name => join(dir, name))
    .map(path => parseStoredContribution(path, readText(path)))
}

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

const citedFile = (path: string, recorded: Map<string, string | undefined>): CitedFile =>
  existsSync(path) ? {kind: 'present', path, sha256: hashFile(path)} : {kind: 'gone', path, recordedSha256: recorded.get(path)}

async function uploadOne(uploader: IUploader, path: string): Promise<string> {
  try {
    return await uploader.upload(path)
  } catch (error) {
    if (error instanceof DraftReviewError) throw error
    throw new DraftReviewError(`uploading ${path} failed: ${message(error)}`, EXIT.unavailable)
  }
}

export type PreparedUploads = {cited: CitedFile[]; plan: UploadPlan; uploader: IUploader | undefined}

export function prepareUploads(store: PrStore, head: string, paths: readonly string[], uploaderFor: (firstPath: string) => IUploader): PreparedUploads {
  const recorded = recordedHashes(store, head)
  const cited = paths.map(path => citedFile(path, recorded))
  const plan = planUploads(cited, readLedger(store))
  const [missing] = plan.missingFiles
  if (missing !== undefined) {
    throw new DraftReviewError(`the review cites ${missing}, which no longer exists, and the ledger has no upload of the content that add recorded for it`, EXIT.noInput)
  }
  const [first] = plan.toUpload
  return {cited, plan, uploader: first === undefined ? undefined : uploaderFor(first.path)}
}

export async function runUploads(store: PrStore, prepared: PreparedUploads): Promise<Map<string, string>> {
  const {cited, plan, uploader} = prepared
  if (uploader === undefined) return plan.urlByPath
  for (const file of plan.toUpload) {
    const url = await uploadOne(uploader, file.path)
    writeJson(ledgerPath(store), recordUpload(readLedger(store), {...file, url}))
  }
  const final = planUploads(cited, readLedger(store))
  if (final.toUpload.length > 0) throw new DraftReviewError(`the ledger ${ledgerPath(store)} did not record every upload`, EXIT.ioErr)
  return final.urlByPath
}

export const readDraftRecord = (store: PrStore): DraftRecord | undefined => readStoredJson(join(store.prDir, DRAFT_FILE), DraftRecordSchema)

export const writeDraftRecord = (store: PrStore, record: DraftRecord): void => writeJson(join(store.prDir, DRAFT_FILE), record)

export function writeSynthesized(store: PrStore, head: string, text: string): string {
  const path = join(headDir(store, head), SYNTHESIZED_FILE)
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
