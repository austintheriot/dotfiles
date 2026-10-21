import {existsSync, readFileSync} from 'node:fs'
import {join} from 'node:path'

export const PAUSE_FILE = 'paused'

export function recordingActive(dir: string): boolean {
  const path = join(dir, 'recording.json')
  if (!existsSync(path) || existsSync(join(dir, PAUSE_FILE))) return false
  try {
    const state: unknown = JSON.parse(readFileSync(path, 'utf8'))
    return typeof state === 'object' && state !== null && !('stoppedAt' in state)
  } catch {
    return false
  }
}

export function mayStartCapture(state: {stopRequested: boolean; paused: boolean; capturing: boolean}): boolean {
  return !state.stopRequested && !state.paused && !state.capturing
}

export const RECORDER_REFUSED_EXIT = 65
export const FIRST_FRAME_BYTES = 4096
export type CaptureFile = {name: string; size: number}

// screencapture writes its movie to a temporary file and moves it into place only at stop. The new
// file holds a header of a few bytes until the first frame arrives.
export function firstFrameWritten(before: ReadonlySet<string>, listing: readonly CaptureFile[]): boolean {
  return listing.some(file => !before.has(file.name) && file.size >= FIRST_FRAME_BYTES)
}
