import {describe, expect, test} from 'bun:test'
import {CHROME_ARGS} from './chrome'

describe('CHROME_ARGS', () => {
  test('opens the window at the origin and at the VM screen size', () => {
    expect(CHROME_ARGS).toContain('--window-position=0,0')
    expect(CHROME_ARGS).toContain('--window-size=1680,1050')
  })

  test('passes no URL, so the startup policy opens the only tab', () => {
    expect(CHROME_ARGS.every(arg => arg.startsWith('--'))).toBe(true)
  })

  test('drops the fake media UI flag, which shows an unsupported-flag infobar', () => {
    expect(CHROME_ARGS).not.toContain('--use-fake-ui-for-media-stream')
  })
})
