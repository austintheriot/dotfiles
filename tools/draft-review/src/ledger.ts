import {z} from 'zod'

export const Sha256Schema = z.string().regex(/^[0-9a-f]{64}$/)

export const LedgerSchema = z.object({entries: z.array(z.object({path: z.string().min(1), sha256: Sha256Schema, url: z.string().min(1)}).strict())}).strict()
export type Ledger = z.infer<typeof LedgerSchema>
export type LedgerEntry = Ledger['entries'][number]

export type RecordedHash = {kind: 'none'} | {kind: 'recorded'; sha256: string} | {kind: 'conflict'; hashes: string[]}
export type CitedFile = {path: string; observedSha256: string | undefined; recorded: RecordedHash}
export type FileToUpload = {path: string; sha256: string}
export type UnresolvedReason = 'gone' | 'changed' | 'conflict'
export type UnresolvedFile = {path: string; reason: UnresolvedReason}
export type UploadPlan = {urlByPath: Map<string, string>; toUpload: FileToUpload[]; unresolved: UnresolvedFile[]}

export const EMPTY_LEDGER: Ledger = {entries: []}

const urlForContent = (ledger: Ledger, sha256: string) => ledger.entries.find(entry => entry.sha256 === sha256)?.url

type Resolution = {kind: 'url'; url: string} | {kind: 'upload'; sha256: string} | {kind: 'unresolved'; reason: UnresolvedReason}

// The recorded hash is the content that the contribution's author saw at add time, so it wins over whatever the path holds now.
function resolve(file: CitedFile, ledger: Ledger): Resolution {
  if (file.recorded.kind === 'conflict') return {kind: 'unresolved', reason: 'conflict'}
  const wanted = file.recorded.kind === 'recorded' ? file.recorded.sha256 : file.observedSha256
  if (wanted === undefined) return {kind: 'unresolved', reason: 'gone'}
  const url = urlForContent(ledger, wanted)
  if (url !== undefined) return {kind: 'url', url}
  if (file.observedSha256 === wanted) return {kind: 'upload', sha256: wanted}
  return {kind: 'unresolved', reason: file.observedSha256 === undefined ? 'gone' : 'changed'}
}

export function planUploads(cited: readonly CitedFile[], ledger: Ledger): UploadPlan {
  const resolved = cited.map(file => ({path: file.path, resolution: resolve(file, ledger)}))
  const uploadable = resolved.flatMap(({path, resolution}): FileToUpload[] => (resolution.kind === 'upload' ? [{path, sha256: resolution.sha256}] : []))
  return {
    urlByPath: new Map(resolved.flatMap(({path, resolution}): [string, string][] => (resolution.kind === 'url' ? [[path, resolution.url]] : []))),
    toUpload: uploadable.filter((file, index) => uploadable.findIndex(other => other.sha256 === file.sha256) === index),
    unresolved: resolved.flatMap(({path, resolution}): UnresolvedFile[] => (resolution.kind === 'unresolved' ? [{path, reason: resolution.reason}] : []))
  }
}

export const recordUpload = (ledger: Ledger, entry: LedgerEntry): Ledger => ({entries: [...ledger.entries, entry]})
