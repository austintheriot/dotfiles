import {describe, expect, test} from 'bun:test'
import {strayChromePids} from './guest'

const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const PROFILE = '/Users/admin/qa-chrome-profile'

describe('strayChromePids', () => {
  test('returns the main Chrome process that does not use the QA profile', () => {
    const ps = [`  101 ${CHROME}`, `  202 ${CHROME} --user-data-dir=${PROFILE} --remote-debugging-port=9333`].join('\n')
    expect(strayChromePids(ps, PROFILE)).toEqual([101])
  })

  test('keeps Chrome that runs the QA profile', () => {
    expect(strayChromePids(`  202 ${CHROME} --no-first-run --user-data-dir=${PROFILE} about:blank`, PROFILE)).toEqual([])
  })

  test('treats a different profile directory as stray', () => {
    expect(strayChromePids(`  303 ${CHROME} --user-data-dir=${PROFILE}-old`, PROFILE)).toEqual([303])
  })

  test('ignores Chrome helper processes and non-Chrome processes', () => {
    const helper = '/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/1/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer) --type=renderer'
    const ps = [`  1 /sbin/launchd`, `  404 ${helper}`, `  505 /usr/bin/grep Google Chrome`].join('\n')
    expect(strayChromePids(ps, PROFILE)).toEqual([])
  })

  test('returns every stray process in order and skips blank lines', () => {
    const ps = `\n  11 ${CHROME}\n\n  12 ${CHROME} --incognito\n`
    expect(strayChromePids(ps, PROFILE)).toEqual([11, 12])
  })

  test('returns nothing for empty output', () => {
    expect(strayChromePids('', PROFILE)).toEqual([])
  })
})
