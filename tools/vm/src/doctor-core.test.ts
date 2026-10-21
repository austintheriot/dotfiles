import {describe, expect, test} from 'bun:test'
import {bunTooOld, diagnose as diagnoseAt, type HostFacts} from './doctor-core'

const now = new Date('2026-10-14T12:00:00.000Z')
const marker = (at = '2026-10-10T10:00:00.000Z') => ({at, inputs: {}})
const diagnose = (facts: HostFacts) => diagnoseAt(facts, now)

const healthy: HostFacts = {
  arch: 'arm64',
  bunVersion: '1.3.14',
  tartVersion: '2.32.1',
  softnet: {owner: 'root', setuid: true},
  ffmpeg: true,
  ffprobe: true,
  chrome: true,
  baseImage: true,
  onPath: true,
  image: {steps: {provision: marker(), grant: marker(), approve: marker(), signin: marker()}, google: {state: 'signed-in', at: '2026-10-10T10:00:00.000Z'}},
  sessionStored: true,
  profile: {kind: 'file', path: '/home/me/.claude/local/vm/profile.json'}
}

const profileCheck = (facts: HostFacts) => diagnose(facts).find(check => check.name === 'profile')

describe('bunTooOld', () => {
  test('compares versions number by number', () => {
    expect(bunTooOld('1.2.0', '1.3.0')).toBe(true)
    expect(bunTooOld('1.10.0', '1.3.0')).toBe(false)
    expect(bunTooOld('1.3.0', '1.3.0')).toBe(false)
  })
})

describe('diagnose', () => {
  test('passes every check on a ready Mac', () => {
    expect(diagnose(healthy).every(check => check.ok)).toBe(true)
  })

  test('names a fix for each missing piece', () => {
    const failed = diagnose({...healthy, tartVersion: undefined, ffprobe: false, baseImage: false})
      .filter(check => !check.ok)
      .map(check => [check.name, check.fix])
    expect(failed).toEqual([
      ['tart', 'brew install cirruslabs/cli/tart'],
      ['ffmpeg', 'brew install ffmpeg'],
      ['base image', 'vm image build (see setup/README.md)']
    ])
  })

  test('refuses a Softnet that is not setuid root, because Tart cannot isolate the VM network without it', () => {
    const softnet = diagnose({...healthy, softnet: {owner: 'austin', setuid: false}}).find(check => check.name === 'softnet')
    expect(softnet).toMatchObject({ok: false, fix: 'setup/install-softnet.sh (asks for your sudo password once)'})
  })

  test('refuses an Intel Mac, because Tart runs macOS guests only on Apple silicon', () => {
    expect(diagnose({...healthy, arch: 'x86_64'}).find(check => check.name === 'Apple silicon')?.ok).toBe(false)
  })

  test('shows which profile file vm reads', () => {
    expect(profileCheck(healthy)).toMatchObject({ok: true, detail: '/home/me/.claude/local/vm/profile.json'})
  })

  test('accepts a missing profile, because the defaults work for any checkout', () => {
    expect(profileCheck({...healthy, profile: {kind: 'defaults', path: '/p.json'}})).toMatchObject({ok: true, detail: 'no /p.json, so vm copies the whole checkout and adds no project setup'})
  })

  test('refuses an invalid profile and names the reason', () => {
    expect(profileCheck({...healthy, profile: {kind: 'invalid', path: '/p.json', reason: 'worktree.paths: too_small'}})).toMatchObject({
      ok: false,
      detail: '/p.json is invalid: worktree.paths: too_small'
    })
  })

  test('flags a vm command that is not on PATH', () => {
    expect(diagnose({...healthy, onPath: false}).find(check => check.name === 'vm on PATH')).toMatchObject({ok: false, fix: './install.sh'})
  })

  test('names the fix for a missing image step marker', () => {
    const {approve, ...steps} = healthy.image.steps
    expect(diagnose({...healthy, image: {...healthy.image, steps}}).find(check => check.name === 'image approve')).toMatchObject({ok: false, fix: 'vm image approve'})
  })

  test('refuses a missing grant marker with vm image grant', () => {
    const {grant, ...steps} = healthy.image.steps
    expect(diagnose({...healthy, image: {...healthy.image, steps}}).find(check => check.name === 'image grant')).toMatchObject({ok: false, fix: 'vm image grant'})
  })

  test('does not check warm, because it is optional', () => {
    expect(diagnose(healthy).some(check => check.name === 'image warm')).toBe(false)
  })

  test('refuses a signed-out Google session', () => {
    const image = {...healthy.image, google: {state: 'signed-out' as const, at: '2026-10-10T10:00:00.000Z'}}
    expect(diagnose({...healthy, image}).find(check => check.name === 'Google session')).toMatchObject({ok: false, fix: 'vm image signin'})
  })

  test('refuses a missing sealed session', () => {
    expect(diagnose({...healthy, sessionStored: false}).find(check => check.name === 'Google session')).toMatchObject({ok: false, fix: 'vm image signin'})
  })

  test('refuses a Google session that was never checked', () => {
    const {google, ...image} = healthy.image
    expect(diagnose({...healthy, image}).find(check => check.name === 'Google session')).toMatchObject({ok: false, detail: expect.stringContaining('never checked')})
  })

  test('refuses an approve marker older than 28 days and names its age', () => {
    const steps = {...healthy.image.steps, approve: marker('2026-09-14T12:00:00.000Z')}
    expect(diagnose({...healthy, image: {...healthy.image, steps}}).find(check => check.name === 'image approve')).toMatchObject({
      ok: false,
      detail: expect.stringContaining('30 days'),
      fix: 'vm image approve'
    })
  })

  test('keeps an approve marker that is 27 days old', () => {
    const steps = {...healthy.image.steps, approve: marker('2026-09-17T12:00:00.000Z')}
    expect(diagnose({...healthy, image: {...healthy.image, steps}}).find(check => check.name === 'image approve')?.ok).toBe(true)
  })
})
