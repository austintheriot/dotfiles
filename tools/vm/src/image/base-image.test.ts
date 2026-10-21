import {describe, expect, test} from 'bun:test'
import {baseImageState, MARKER_LISTING_SCRIPT, parseHostImageState, parseMarkerListing} from './base-image'

describe('baseImageState', () => {
  test('reads tart get output', () => {
    expect(baseImageState({code: 0, stdout: '{"State":"running","CPU":4}', stderr: ''})).toBe('running')
    expect(baseImageState({code: 0, stdout: '{"State":"stopped"}', stderr: ''})).toBe('stopped')
  })

  test('treats a failed lookup as missing', () => {
    expect(baseImageState({code: 1, stdout: '', stderr: 'VM "pr-qa-base" not found'})).toBe('missing')
  })

  test('treats unreadable output as running, so nothing boots twice', () => {
    expect(baseImageState({code: 0, stdout: 'garbage', stderr: ''})).toBe('running')
  })
})

describe('parseMarkerListing', () => {
  test('reads one step per line', () => {
    const listing = ['grant\t{"at":"2026-10-14T10:00:00.000Z","inputs":{"agent":"/opt/a"}}', 'signin\t{"at":"2026-10-14T11:00:00.000Z","inputs":{}}', ''].join('\n')
    expect(parseMarkerListing(listing)).toEqual({
      grant: {at: '2026-10-14T10:00:00.000Z', inputs: {agent: '/opt/a'}},
      signin: {at: '2026-10-14T11:00:00.000Z', inputs: {}}
    })
  })

  test('skips unknown steps and broken lines', () => {
    expect(parseMarkerListing('other\t{"at":"x","inputs":{}}\ngrant\tnot json\n')).toEqual({})
  })
})

describe('parseHostImageState', () => {
  test('starts empty without a file', () => {
    expect(parseHostImageState(undefined)).toEqual({steps: {}})
  })

  test('starts empty on a broken file', () => {
    expect(parseHostImageState('{')).toEqual({steps: {}})
  })

  test('keeps steps and the Google state', () => {
    const state = {steps: {grant: {at: 'x', inputs: {}}}, google: {state: 'signed-in' as const, at: 'y'}}
    expect(parseHostImageState(JSON.stringify(state))).toEqual(state)
  })
})

describe('MARKER_LISTING_SCRIPT', () => {
  test('hands zsh a $\'\\t\' quote, which prints a real tab that print -r keeps', () => {
    expect(MARKER_LISTING_SCRIPT).toContain('"${file:t}"$\'\\t\'"$(<$file)"')
    expect(MARKER_LISTING_SCRIPT).not.toContain('\t')
  })
})
