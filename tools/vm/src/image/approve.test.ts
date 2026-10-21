import {describe, expect, test} from 'bun:test'
import {isConfirmed, parseLastAlerted, planApprove, remainingPrompts, terminalRefusal} from './approve'

describe('remainingPrompts', () => {
  test('lists both prompts when neither is scripted', () => {
    expect(remainingPrompts({replayd: false, localNetwork: false})).toHaveLength(2)
  })

  test('lists nothing when both are scripted', () => {
    expect(remainingPrompts({replayd: true, localNetwork: true})).toEqual([])
  })

  test('names the local network prompt when only replayd is scripted', () => {
    const prompts = remainingPrompts({replayd: true, localNetwork: false})
    expect(prompts).toHaveLength(1)
    expect(prompts[0]).toContain('find devices on local networks')
  })

  test('names the guest agent when only the local network prompt is scripted', () => {
    const prompts = remainingPrompts({replayd: false, localNetwork: true})
    expect(prompts).toHaveLength(1)
    expect(prompts[0]).toContain('tart-guest-agent')
  })
})

describe('terminalRefusal', () => {
  test('refuses without a terminal, with exit 69 and the command to run', () => {
    const refusal = terminalRefusal('signin', false)
    expect(refusal?.exitCode).toBe(69)
    expect(refusal?.message).toBe('needs a terminal: run vm image signin yourself')
  })

  test('allows a terminal', () => {
    expect(terminalRefusal('approve', true)).toBeUndefined()
  })
})

describe('isConfirmed', () => {
  test.each(['y', 'Y', 'yes', 'YES', ' Yes '])('accepts %p', answer => {
    expect(isConfirmed(answer)).toBe(true)
  })

  test.each(['', 'n', 'no', 'yep', 'ye', 'y y'])('rejects %p', answer => {
    expect(isConfirmed(answer)).toBe(false)
  })

  test('treats end of input as no', () => {
    expect(isConfirmed(undefined)).toBe(false)
  })
})

describe('parseLastAlerted', () => {
  test('reads the date plutil prints', () => {
    expect(parseLastAlerted('2026-10-01T09:30:00Z\n')?.toISOString()).toBe('2026-10-01T09:30:00.000Z')
  })

  test('refuses an error message or an empty read', () => {
    expect(parseLastAlerted('')).toBeUndefined()
    expect(parseLastAlerted('Could not extract value, error: No value at that key path')).toBeUndefined()
  })
})

describe('planApprove', () => {
  const now = new Date('2026-10-15T12:00:00Z')
  const scripted = {replayd: false, localNetwork: false}

  test('asks for nothing while the approval is current and the local network prompt was answered', () => {
    expect(planApprove({scripted, lastAlerted: new Date('2026-10-01T12:00:00Z'), approvedBefore: true, now}).prompts).toEqual([])
  })

  test('asks for the screen-capture approval again once it is older than 28 days', () => {
    const prompts = planApprove({scripted, lastAlerted: new Date('2026-09-10T12:00:00Z'), approvedBefore: true, now}).prompts
    expect(prompts).toHaveLength(1)
    expect(prompts[0]).toContain('tart-guest-agent')
  })

  test('asks again when the replayd date lies in the future', () => {
    const prompts = planApprove({scripted, lastAlerted: new Date('2026-10-20T12:00:00Z'), approvedBefore: true, now}).prompts
    expect(prompts).toHaveLength(1)
  })

  test('asks for both on an image that was never approved', () => {
    expect(planApprove({scripted, lastAlerted: undefined, approvedBefore: false, now}).prompts).toHaveLength(2)
  })
})
