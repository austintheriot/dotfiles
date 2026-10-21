import {describe, expect, test} from 'bun:test'
import {expiryMessage, planSignin, signinGuide} from './signin'

const record = {version: 1 as const, exportedAt: 'x', account: 'austin', cookies: []}

describe('planSignin', () => {
  test('restores a stored session when restore is enabled', () => {
    expect(planSignin({kind: 'found', record}, true)).toEqual({restore: true, warning: undefined})
  })

  test('goes to the guided sign-in when restore is disabled', () => {
    expect(planSignin({kind: 'found', record}, false)).toEqual({restore: false, warning: undefined})
  })

  test('goes to the guided sign-in when nothing is stored', () => {
    expect(planSignin({kind: 'missing'}, true)).toEqual({restore: false, warning: undefined})
  })

  test('names why a stored session was not used', () => {
    expect(planSignin({kind: 'invalid', reason: 'x.enc: the key does not open it, or it was changed'}, true)).toEqual({
      restore: false,
      warning: 'stored Google session not used: x.enc: the key does not open it, or it was changed'
    })
  })
})

describe('expiryMessage', () => {
  test('says expired when the last check found the session signed in', () => {
    expect(expiryMessage('signed-out', {vm: 'pr-qa-base', previous: 'signed-in'})).toEqual({
      level: 'warn',
      text: 'Google session in pr-qa-base expired: it was signed in at the last check. Run vm image signin.'
    })
  })

  test('says signed out, not expired, when nothing says it was signed in', () => {
    const expected = 'Google session in pr-qa-7 is signed out. Run vm image signin.'
    expect(expiryMessage('signed-out', {vm: 'pr-qa-7', previous: undefined})?.text).toBe(expected)
    expect(expiryMessage('signed-out', {vm: 'pr-qa-7', previous: 'signed-out'})?.text).toBe(expected)
  })

  test('names the VM it probed when the state is unknown', () => {
    expect(expiryMessage('unknown', {vm: 'pr-qa-7', previous: 'signed-in'})).toEqual({
      level: 'warn',
      text: 'could not tell whether the Google session in pr-qa-7 is signed in'
    })
  })

  test('says nothing when signed in', () => {
    expect(expiryMessage('signed-in', {vm: 'pr-qa-7', previous: 'signed-out'})).toBeUndefined()
  })
})

describe('signinGuide', () => {
  test('opens only the Google sign-in when the profile adds nothing', () => {
    expect(signinGuide({tabs: [], steps: []})).toEqual({
      tabs: ['https://accounts.google.com'],
      lines: ['In the VM window:', '  1. Sign in to Google with your account in the accounts.google.com tab.'],
      prompt: 'Press Enter here when you are signed in... '
    })
  })

  test('opens the profile tabs after the Google tab and numbers the profile steps after the Google step', () => {
    const guide = signinGuide({tabs: ['https://staging.example.com/app'], steps: ['Click "Sign in with Google" in the staging tab.']})
    expect(guide.tabs).toEqual(['https://accounts.google.com', 'https://staging.example.com/app'])
    expect(guide.lines.slice(1)).toEqual([
      '  1. Sign in to Google with your account in the accounts.google.com tab.',
      '  2. Click "Sign in with Google" in the staging tab.'
    ])
    expect(guide.prompt).toBe('Press Enter here when every tab has loaded... ')
  })
})
