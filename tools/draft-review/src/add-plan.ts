import {contributionFileName, staleRoundTwoNames} from './clear'
import type {Contribution} from './contribution'
import {validateContributions} from './contribution-set'

export type StoredFile = {name: string; contribution: Contribution}
export type AddInput = {contribution: Contribution; listing: readonly string[]; stored: readonly StoredFile[]}
export type AddPlan = {kind: 'store'; name: string; remove: string[]} | {kind: 'refuse'; reason: string}

// The check runs on the set that the store holds after the add, so a file that breaks a merge rule never reaches show or create.
export function planAdd(input: AddInput): AddPlan {
  const {contribution, listing, stored} = input
  const name = contributionFileName(contribution)
  const remove = staleRoundTwoNames(contribution.kind, listing)
  const kept = stored.filter(file => file.name !== name && !remove.includes(file.name)).map(file => file.contribution)
  const set = validateContributions([...kept, contribution])
  return set.ok ? {kind: 'store', name, remove} : {kind: 'refuse', reason: set.reason}
}
