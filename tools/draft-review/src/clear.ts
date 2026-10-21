export const CONTRIBUTION_FILE_NAME = /^(code-review|dev-qa|dev-qa-verify|code-trace)(@[^/]+)?\.json$/

export const FILE_HASHES_SUFFIX = '.files'

const isContribution = (name: string) => CONTRIBUTION_FILE_NAME.test(name)

const isHashRecord = (name: string) => name.endsWith(FILE_HASHES_SUFFIX) && isContribution(name.slice(0, -FILE_HASHES_SUFFIX.length))

export const namesToClear = (listing: readonly string[]): string[] => listing.filter(name => isContribution(name) || isHashRecord(name)).sort()
