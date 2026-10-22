import type {Contribution} from './contribution'

export const CONTRIBUTION_FILE_NAME = /^(code-review|dev-qa|dev-qa-verify|code-trace)(@[^/]+)?\.json$/

export const FILE_HASHES_SUFFIX = '.files'

// A platform label is free text but never contains @, so @ keeps `dev-qa` with platform `verify-x` apart from `dev-qa-verify` with platform `x`.
export const contributionFileName = (contribution: Contribution): string => {
  const platform = contribution.kind === 'dev-qa' || contribution.kind === 'dev-qa-verify' ? contribution.platform : undefined
  return platform === undefined ? `${contribution.kind}.json` : `${contribution.kind}@${encodeURIComponent(platform)}.json`
}

const isContribution = (name: string) => CONTRIBUTION_FILE_NAME.test(name)

const isHashRecord = (name: string) => name.endsWith(FILE_HASHES_SUFFIX) && isContribution(name.slice(0, -FILE_HASHES_SUFFIX.length))

export const namesToClear = (listing: readonly string[]): string[] => listing.filter(name => isContribution(name) || isHashRecord(name)).sort()

const ROUND_TWO_ANSWERING: Record<Contribution['kind'], RegExp | undefined> = {
  'code-review': /^(dev-qa-verify(@[^/]+)?|code-trace)\.json$/,
  'dev-qa': /^code-trace\.json$/,
  'dev-qa-verify': undefined,
  'code-trace': undefined
}

// Finding ids restart at R1 and Q1 on every run, so a verdict kept from an earlier round can name a different finding with the same id.
// A code-trace verdict names review ids through duplicateOf, so a new code-review removes the code-trace too.
export function staleRoundTwoNames(kind: Contribution['kind'], listing: readonly string[]): string[] {
  const answering = ROUND_TWO_ANSWERING[kind]
  if (answering === undefined) return []
  const contributionOf = (name: string) => (name.endsWith(FILE_HASHES_SUFFIX) ? name.slice(0, -FILE_HASHES_SUFFIX.length) : name)
  return listing.filter(name => answering.test(contributionOf(name))).sort()
}
