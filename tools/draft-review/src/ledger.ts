import {z} from 'zod'
import {isDefined} from './lists'

export const Sha256Schema = z.string().regex(/^[0-9a-f]{64}$/)

export const LedgerSchema = z.object({entries: z.array(z.object({path: z.string().min(1), sha256: Sha256Schema, url: z.string().min(1)}).strict())}).strict()
export type Ledger = z.infer<typeof LedgerSchema>
export type LedgerEntry = Ledger['entries'][number]

export type CitedFile = {kind: 'present'; path: string; sha256: string} | {kind: 'gone'; path: string; recordedSha256: string | undefined}
export type FileToUpload = {path: string; sha256: string}
export type UploadPlan = {urlByPath: Map<string, string>; toUpload: FileToUpload[]; missingFiles: string[]}

export const EMPTY_LEDGER: Ledger = {entries: []}

const contentHash = (file: CitedFile) => (file.kind === 'present' ? file.sha256 : file.recordedSha256)
const urlForContent = (ledger: Ledger, sha256: string | undefined) =>
  sha256 === undefined ? undefined : ledger.entries.find(entry => entry.sha256 === sha256)?.url

export function planUploads(cited: readonly CitedFile[], ledger: Ledger): UploadPlan {
  const resolved = cited.map(file => ({file, url: urlForContent(ledger, contentHash(file))}))
  const unknown = resolved.filter(item => item.url === undefined).map(item => item.file)
  const uploadable = unknown.flatMap((file): FileToUpload[] => (file.kind === 'present' ? [{path: file.path, sha256: file.sha256}] : []))
  return {
    urlByPath: new Map(resolved.flatMap(({file, url}): [string, string][] => (isDefined(url) ? [[file.path, url]] : []))),
    toUpload: uploadable.filter((file, index) => uploadable.findIndex(other => other.sha256 === file.sha256) === index),
    missingFiles: unknown.filter(file => file.kind === 'gone').map(file => file.path)
  }
}

export const recordUpload = (ledger: Ledger, entry: LedgerEntry): Ledger => ({entries: [...ledger.entries, entry]})
