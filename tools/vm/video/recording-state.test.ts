import {describe, expect, test} from 'bun:test'
import {mkdtempSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {FIRST_FRAME_BYTES, firstFrameWritten, mayStartCapture, PAUSE_FILE, recordingActive} from './recording-state'

function recordingDir(recording: object | undefined, paused = false): string {
  const dir = mkdtempSync(join(tmpdir(), 'qa-video-state-'))
  if (recording) writeFileSync(join(dir, 'recording.json'), JSON.stringify(recording))
  if (paused) writeFileSync(join(dir, PAUSE_FILE), JSON.stringify({at: 1, reason: 'human sign-in'}))
  return dir
}

describe('recordingActive', () => {
  test('is true while a recording runs', () => {
    expect(recordingActive(recordingDir({step: '1'}))).toBe(true)
  })

  test('is false with no recording, and after stop', () => {
    expect(recordingActive(recordingDir(undefined))).toBe(false)
    expect(recordingActive(recordingDir({step: '1', stoppedAt: 5}))).toBe(false)
  })

  test('is false while paused, so no input typed during a pause reaches the video', () => {
    expect(recordingActive(recordingDir({step: '1'}, true))).toBe(false)
  })
})

describe('mayStartCapture', () => {
  const idle = {stopRequested: false, paused: false, capturing: false}

  test('allows a restart only when nothing stops it and nothing runs', () => {
    expect(mayStartCapture(idle)).toBe(true)
    expect(mayStartCapture({...idle, stopRequested: true})).toBe(false)
    expect(mayStartCapture({...idle, capturing: true})).toBe(false)
  })

  test('refuses a restart while paused', () => {
    expect(mayStartCapture({...idle, paused: true})).toBe(false)
  })
})

describe('firstFrameWritten', () => {
  const before = new Set(['stale.mov'])

  test('is false until the new capture file exists', () => {
    expect(firstFrameWritten(before, [])).toBe(false)
    expect(firstFrameWritten(before, [{name: 'stale.mov', size: 900_000}])).toBe(false)
  })

  test('is false while the new file holds only the movie header', () => {
    expect(firstFrameWritten(before, [{name: 'new.mov', size: 0}])).toBe(false)
    expect(firstFrameWritten(before, [{name: 'new.mov', size: 36}])).toBe(false)
  })

  test('is true once the new file holds a frame', () => {
    expect(firstFrameWritten(before, [{name: 'new.mov', size: FIRST_FRAME_BYTES}])).toBe(true)
    expect(firstFrameWritten(before, [{name: 'stale.mov', size: 10}, {name: 'new.mov', size: 86_213}])).toBe(true)
  })
})
