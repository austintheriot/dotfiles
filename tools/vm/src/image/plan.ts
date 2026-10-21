import {BASE_IMAGE} from '../config'
import {IMAGE_STEPS, type ImageStep, type Marker, type Markers, type StepInputs} from './base-image'

export const APPROVE_MAX_AGE_DAYS = 28
const DAY_MS = 24 * 60 * 60 * 1000

export const markerAgeDays = (at: string, now: Date): number => (now.getTime() - Date.parse(at)) / DAY_MS

export const isCurrentAge = (age: number): boolean => age >= 0 && age <= APPROVE_MAX_AGE_DAYS

export function approvalWarning(marker: Marker | undefined, now: Date): string | undefined {
  const age = marker === undefined ? undefined : markerAgeDays(marker.at, now)
  if (age !== undefined && isCurrentAge(age)) return undefined
  const state = age === undefined ? 'missing' : Number.isFinite(age) ? `${Math.floor(age)} days old` : 'unreadable'
  return `WARNING: the screen-capture approval in ${BASE_IMAGE} is ${state}; recordings can show a permission prompt. Run vm image approve.`
}

const sameInputs = (left: StepInputs, right: StepInputs) => {
  const keys = new Set([...Object.keys(left), ...Object.keys(right)])
  return [...keys].every(key => left[key] === right[key])
}

export function planImageBuild(
  markers: Markers,
  current: Partial<Record<ImageStep, StepInputs>>,
  options: {checkout: boolean; now: Date}
): {run: ImageStep[]; skipped: {step: ImageStep; reason: string}[]} {
  const run: ImageStep[] = []
  const skipped: {step: ImageStep; reason: string}[] = []
  let provisioned = false
  for (const step of IMAGE_STEPS) {
    const marker = markers[step]
    const inputs = current[step]
    const lapsed = step === 'approve' && marker !== undefined && !isCurrentAge(markerAgeDays(marker.at, options.now))
    const due = step === 'signin' || provisioned || marker === undefined || lapsed || (inputs !== undefined && !sameInputs(marker.inputs, inputs))
    if (!due) continue
    if (step === 'warm' && !options.checkout) {
      skipped.push({step, reason: 'no --checkout given'})
      continue
    }
    run.push(step)
    if (step === 'provision') provisioned = true
  }
  return {run, skipped}
}
