import {describe, expect, test} from 'bun:test'
import {RECORDER_REFUSED_EXIT} from '../video/recording-state'
import {stopFailure} from './recorder'

describe('stopFailure', () => {
  test('adds no message when the recorder refused and printed its reason', () => {
    const error = stopFailure('pr-qa-1', RECORDER_REFUSED_EXIT)
    expect(error.message).toBe('')
    expect(error.exitCode).toBe(RECORDER_REFUSED_EXIT)
  })

  test('says the recorder did not stop cleanly when it crashed', () => {
    const error = stopFailure('pr-qa-1', 1)
    expect(error.message).toBe('the recorder in pr-qa-1 did not stop cleanly')
    expect(error.exitCode).toBe(1)
  })
})
