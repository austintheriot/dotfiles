import {describe, expect, test} from 'bun:test'
import {approvalWarning, planImageBuild} from './plan'

const now = new Date('2026-10-14T12:00:00.000Z')
const marker = (inputs: Record<string, string> = {}, at = '2026-10-14T10:00:00.000Z') => ({at, inputs})
const daysBefore = (days: number) => new Date(now.getTime() - days * 24 * 60 * 60 * 1000).toISOString()
const done = {provision: marker({bun: '1.3.14'}), grant: marker({agent: '/a', localNetwork: 'false'}), approve: marker({chrome: '141'}), warm: marker({lock: 'aa'}), signin: marker()}
const current = {provision: {bun: '1.3.14'}, grant: {agent: '/a', localNetwork: 'false'}, approve: {chrome: '141'}, warm: {lock: 'aa'}}
const options = {checkout: true, now}

describe('planImageBuild', () => {
  test('runs only signin on a current image', () => {
    expect(planImageBuild(done, current, options)).toEqual({run: ['signin'], skipped: []})
  })

  test('runs every step on a fresh image, skipping warm without a checkout', () => {
    expect(planImageBuild({}, {}, {checkout: false, now})).toEqual({run: ['provision', 'grant', 'approve', 'signin'], skipped: [{step: 'warm', reason: 'no --checkout given'}]})
  })

  test('reruns grant when the agent path changed', () => {
    expect(planImageBuild(done, {...current, grant: {agent: '/b', localNetwork: 'false'}}, options).run).toEqual(['grant', 'signin'])
  })

  test('reruns approve when Chrome changed', () => {
    expect(planImageBuild(done, {...current, approve: {chrome: '142'}}, options).run).toEqual(['approve', 'signin'])
  })

  test('runs every later step after provision', () => {
    expect(planImageBuild({...done, provision: marker({bun: '1.3.0'})}, current, options).run).toEqual(['provision', 'grant', 'approve', 'warm', 'signin'])
  })

  test('skips a due warm without a checkout', () => {
    expect(planImageBuild({...done, warm: marker({lock: 'old'})}, current, {checkout: false, now})).toEqual({run: ['signin'], skipped: [{step: 'warm', reason: 'no --checkout given'}]})
  })

  test('reruns approve when its marker is 29 days old, because the screen-capture approval lapses after 30', () => {
    expect(planImageBuild({...done, approve: marker({chrome: '141'}, daysBefore(29))}, current, options).run).toEqual(['approve', 'signin'])
  })

  test('keeps approve when its marker is 27 days old', () => {
    expect(planImageBuild({...done, approve: marker({chrome: '141'}, daysBefore(27))}, current, options).run).toEqual(['signin'])
  })

  test('reruns approve when its marker date cannot be read', () => {
    expect(planImageBuild({...done, approve: marker({chrome: '141'}, 'garbage')}, current, options).run).toEqual(['approve', 'signin'])
  })
})

describe('approvalWarning', () => {
  test('is silent for an approval younger than the limit', () => {
    expect(approvalWarning(marker({}, daysBefore(27)), now)).toBeUndefined()
  })

  test('names the age of an old approval', () => {
    expect(approvalWarning(marker({}, daysBefore(40)), now)).toBe(
      'WARNING: the screen-capture approval in pr-qa-base is 40 days old; recordings can show a permission prompt. Run vm image approve.'
    )
  })

  test('says missing when there is no marker', () => {
    expect(approvalWarning(undefined, now)).toBe(
      'WARNING: the screen-capture approval in pr-qa-base is missing; recordings can show a permission prompt. Run vm image approve.'
    )
  })

  test('says unreadable when the marker date does not parse', () => {
    expect(approvalWarning(marker({}, 'not a date'), now)).toContain('is unreadable;')
  })
})

describe('approval dates in the future', () => {
  const future = new Date(now.getTime() + 2 * 24 * 60 * 60 * 1000).toISOString()

  test('does not count a future approval date as current', () => {
    expect(approvalWarning(marker({}, future), now)).toContain('screen-capture approval')
  })

  test('reruns approve when its marker date lies in the future', () => {
    expect(planImageBuild({...done, approve: marker({chrome: '141'}, future)}, current, options).run).toEqual(['approve', 'signin'])
  })
})
