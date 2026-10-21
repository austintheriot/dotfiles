import {z} from 'zod'
import {isDefined} from './lists'

const Sha256 = z.string().regex(/^[0-9a-f]{64}$/)

export const LedgerSchema = z.object({entries: z.array(z.object({path: z.string().min(1), sha256: Sha256, url: z.string().min(1)}).strict())}).strict()
export type Ledger = z.infer<typeof LedgerSchema>
export type LedgerEntry = Ledger['entries'][number]

export type CitedFile = {path: string; sha256: string | undefined}
export type FileToUpload = {path: string; sha256: string}
export type UploadPlan = {urlByPath: Map<string, string>; toUpload: FileToUpload[]; missingFiles: string[]}

export const EMPTY_LEDGER: Ledger = {entries: []}

function knownUrl(file: CitedFile, ledger: Ledger): string | undefined {
  const byContent = (sha256: string) => ledger.entries.find(entry => entry.sha256 === sha256)?.url
  const byPath = () => ledger.entries.filter(entry => entry.path === file.path).at(-1)?.url
  return file.sha256 === undefined ? byPath() : byContent(file.sha256)
}

export function planUploads(cited: readonly CitedFile[], ledger: Ledger): UploadPlan {
  const resolved = cited.map(file => ({file, url: knownUrl(file, ledger)}))
  const unknown = resolved.filter(item => item.url === undefined).map(item => item.file)
  const uploadable = unknown.flatMap((file): FileToUpload[] => (file.sha256 === undefined ? [] : [{path: file.path, sha256: file.sha256}]))
  return {
    urlByPath: new Map(resolved.flatMap(({file, url}): [string, string][] => (isDefined(url) ? [[file.path, url]] : []))),
    toUpload: uploadable.filter((file, index) => uploadable.findIndex(other => other.sha256 === file.sha256) === index),
    missingFiles: unknown.filter(file => file.sha256 === undefined).map(file => file.path)
  }
}

export const recordUpload = (ledger: Ledger, entry: LedgerEntry): Ledger => ({entries: [...ledger.entries, entry]})
