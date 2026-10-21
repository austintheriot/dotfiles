import {APPROVE_MAX_AGE_DAYS, markerAgeDays} from './image/plan'
import type {HostImageState, ImageStep} from './image/base-image'
import type {ProfileStatus} from './profile'

export const MIN_BUN = '1.2.0'

export type HostFacts = {
  arch: string
  bunVersion: string
  tartVersion: string | undefined
  softnet: {owner: string; setuid: boolean} | undefined
  ffmpeg: boolean
  ffprobe: boolean
  chrome: boolean
  baseImage: boolean
  onPath: boolean
  image: HostImageState
  sessionStored: boolean
  profile: ProfileStatus
}

export type Check = {name: string; ok: boolean; detail: string; fix: string}

export function bunTooOld(version: string, minimum: string): boolean {
  const parts = (text: string) => text.split('.').map(part => Number.parseInt(part, 10) || 0)
  const [have, need] = [parts(version), parts(minimum)]
  const index = need.findIndex((value, position) => (have[position] ?? 0) !== value)
  return index !== -1 && (have[index] ?? 0) < (need[index] ?? 0)
}

const check = (name: string, ok: boolean, detail: string, fix: string): Check => ({name, ok, detail, fix})

const DOCTOR_STEPS: readonly ImageStep[] = ['provision', 'grant', 'approve', 'signin']

function stepCheck(step: ImageStep, facts: HostFacts, now: Date): Check {
  const marker = facts.image.steps[step]
  const fix = `vm image ${step}`
  if (marker === undefined) return check(`image ${step}`, false, 'not done', fix)
  const age = markerAgeDays(marker.at, now)
  if (step === 'approve' && !(age <= APPROVE_MAX_AGE_DAYS))
    return check(`image ${step}`, false, Number.isFinite(age) ? `done ${Math.floor(age)} days ago, and the approval lapses after ${APPROVE_MAX_AGE_DAYS}` : `done at an unreadable date: ${marker.at}`, fix)
  return check(`image ${step}`, true, `done ${marker.at}`, fix)
}

export function diagnose(facts: HostFacts, now: Date): Check[] {
  const softnetReady = facts.softnet?.owner === 'root' && facts.softnet.setuid
  return [
    check('Apple silicon', facts.arch === 'arm64', facts.arch, 'Tart runs macOS guests only on Apple silicon'),
    profileCheck(facts.profile),
    check('bun', !bunTooOld(facts.bunVersion, MIN_BUN), `${facts.bunVersion} (needs ${MIN_BUN} or later)`, 'bun upgrade'),
    check('tart', facts.tartVersion !== undefined, facts.tartVersion ?? 'not found', 'brew install cirruslabs/cli/tart'),
    check(
      'softnet',
      softnetReady,
      facts.softnet ? `owner ${facts.softnet.owner}, setuid ${facts.softnet.setuid ? 'on' : 'off'}` : 'not found',
      'setup/install-softnet.sh (asks for your sudo password once)'
    ),
    check('ffmpeg', facts.ffmpeg && facts.ffprobe, facts.ffmpeg && facts.ffprobe ? 'found' : 'ffmpeg or ffprobe not found', 'brew install ffmpeg'),
    check('Google Chrome', facts.chrome, facts.chrome ? 'found' : 'not found', 'install Google Chrome: the video editor draws its overlays with it'),
    check('base image', facts.baseImage, facts.baseImage ? 'pr-qa-base' : 'pr-qa-base not found', 'vm image build (see setup/README.md)'),
    ...DOCTOR_STEPS.map(step => stepCheck(step, facts, now)),
    googleSessionCheck(facts),
    check('vm on PATH', facts.onPath, facts.onPath ? 'found' : 'not found', './install.sh')
  ]
}

function profileCheck(profile: ProfileStatus): Check {
  const fix = 'fix the profile: see "Project profile" in the README'
  if (profile.kind === 'invalid') return check('profile', false, `${profile.path} is invalid: ${profile.reason}`, fix)
  if (profile.kind === 'defaults') return check('profile', true, `no ${profile.path}, so vm copies the whole checkout and adds no project setup`, fix)
  return check('profile', true, profile.path, fix)
}

function googleSessionCheck(facts: HostFacts): Check {
  const google = facts.image.google
  const detail = google === undefined ? 'never checked' : `${google.state}, checked ${google.at}`
  const stored = facts.sessionStored ? '' : ', no sealed session on the host'
  return check('Google session', google?.state === 'signed-in' && facts.sessionStored, `${detail}${stored}`, 'vm image signin')
}
