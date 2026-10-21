import {existsSync, mkdirSync, readFileSync, renameSync, writeFileSync} from 'node:fs'
import {dirname} from 'node:path'
import {z} from 'zod'
import {bootVm} from '../boot'
import {quitChrome} from '../chrome'
import {BASE_IMAGE, EXIT, IMAGE_STATE_FILE} from '../config'
import {run, tartExec, VmError, type RunResult} from '../proc'
import type {GoogleSessionState} from './session'

export type ImageStep = 'provision' | 'grant' | 'approve' | 'warm' | 'signin'
export const IMAGE_STEPS: readonly ImageStep[] = ['provision', 'grant', 'approve', 'warm', 'signin']
export type StepInputs = Record<string, string>

const Marker = z.object({at: z.string(), inputs: z.record(z.string())})
export type Marker = z.infer<typeof Marker>
export type Markers = Partial<Record<ImageStep, Marker>>

const isStep = (value: string): value is ImageStep => (IMAGE_STEPS as readonly string[]).includes(value)

export function baseImageState(result: RunResult): 'missing' | 'running' | 'stopped' {
  if (result.code !== 0) return 'missing'
  try {
    return (JSON.parse(result.stdout) as {State?: unknown}).State === 'stopped' ? 'stopped' : 'running'
  } catch {
    return 'running'
  }
}

export function parseMarkerListing(text: string): Markers {
  const markers: Markers = {}
  for (const line of text.split('\n')) {
    const [step = '', json = ''] = line.split('\t')
    if (!isStep(step)) continue
    try {
      const marker = Marker.safeParse(JSON.parse(json))
      if (marker.success) markers[step] = marker.data
    } catch {
      continue
    }
  }
  return markers
}

export const MARKER_LISTING_SCRIPT = 'for file in ~/.qa-image/*(N); do print -r -- "${file:t}"$\'\\t\'"$(<$file)"; done'

export function readMarkers(vm: string): Markers {
  const listing = tartExec(vm, MARKER_LISTING_SCRIPT)
  return listing.code === 0 ? parseMarkerListing(listing.stdout) : {}
}

export function writeMarker(vm: string, step: ImageStep, inputs: StepInputs, now: Date = new Date()): void {
  const written = run(['tart', 'exec', '-i', vm, 'zsh', '-c', `mkdir -p ~/.qa-image && cat > ~/.qa-image/${step}`], {stdin: JSON.stringify({at: now.toISOString(), inputs})})
  if (written.code !== 0) throw new VmError(`could not record step ${step} in ${vm}: ${written.stderr.trim()}`, EXIT.unavailable)
  updateHostImageState(state => ({...state, steps: {...state.steps, [step]: {at: now.toISOString(), inputs}}}))
}

const HostImageState = z.object({
  steps: z.record(Marker).transform(steps => Object.fromEntries(Object.entries(steps).filter(([step]) => isStep(step))) as Markers),
  google: z.object({state: z.enum(['signed-in', 'signed-out', 'unknown']), at: z.string()}).optional()
})
export type HostImageState = {steps: Markers; google?: {state: GoogleSessionState; at: string}}

export function parseHostImageState(text: string | undefined): HostImageState {
  if (text === undefined) return {steps: {}}
  try {
    const parsed = HostImageState.safeParse(JSON.parse(text))
    if (!parsed.success) return {steps: {}}
    return parsed.data.google === undefined ? {steps: parsed.data.steps} : {steps: parsed.data.steps, google: parsed.data.google}
  } catch {
    return {steps: {}}
  }
}

export const readHostImageState = (file: string = IMAGE_STATE_FILE): HostImageState => parseHostImageState(existsSync(file) ? readFileSync(file, 'utf8') : undefined)

export function updateHostImageState(change: (state: HostImageState) => HostImageState, file: string = IMAGE_STATE_FILE): void {
  mkdirSync(dirname(file), {recursive: true})
  const temporary = `${file}.${process.pid}.tmp`
  writeFileSync(temporary, `${JSON.stringify(change(readHostImageState(file)), null, 2)}\n`)
  renameSync(temporary, file)
}

function stopVm(vm: string): void {
  quitChrome(vm)
  run(['tart', 'exec', vm, 'sync'])
  run(['tart', 'stop', '--timeout', '120', vm])
}

// Every exit path stops the image, including Ctrl-C while a step waits for Enter. A running
// pr-qa-base cannot be cloned safely, so a leaked boot breaks every later vm up.
export async function withBaseImage<Result>(options: {window: boolean}, step: (vm: string) => Result | Promise<Result>, vm: string = BASE_IMAGE): Promise<Result> {
  const state = baseImageState(run(['tart', 'get', vm, '--format', 'json']))
  if (state === 'missing')
    throw new VmError(`no ${vm}: run tart clone ghcr.io/cirruslabs/macos-tahoe-base:latest ${vm}, then tart set ${vm} --display 1680x1050pt`, EXIT.noInput)
  if (state === 'running') throw new VmError(`${vm} is running; stop it first`, EXIT.inUse)
  const onInterrupt = () => {
    stopVm(vm)
    process.exit(130)
  }
  const onHangup = () => {
    stopVm(vm)
    process.exit(129)
  }
  process.once('SIGINT', onInterrupt)
  process.once('SIGTERM', onInterrupt)
  process.once('SIGHUP', onHangup)
  try {
    bootVm(vm, {window: options.window, softnet: false})
    return await step(vm)
  } finally {
    process.off('SIGINT', onInterrupt)
    process.off('SIGTERM', onInterrupt)
    process.off('SIGHUP', onHangup)
    stopVm(vm)
  }
}
