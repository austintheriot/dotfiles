import {describe, expect, test} from 'bun:test'
import {parseInvocation, pickPage, settledUrl} from './cdp-session'

describe('parseInvocation', () => {
  test('defaults to the QA Chrome port', () => {
    expect(parseInvocation(['export'])).toEqual({action: 'export', port: 9333, url: undefined, target: undefined})
  })

  test('reads port, page filter, and the URL to open', () => {
    expect(parseInvocation(['import', '--port', '9224', '--url', 'example'])).toEqual({action: 'import', port: 9224, url: 'example', target: undefined})
    expect(parseInvocation(['open', 'https://accounts.google.com'])).toEqual({action: 'open', port: 9333, url: undefined, target: 'https://accounts.google.com'})
  })

  test('returns a usage line for bad input', () => {
    expect(typeof parseInvocation([])).toBe('string')
    expect(typeof parseInvocation(['steal'])).toBe('string')
    expect(typeof parseInvocation(['export', '--port'])).toBe('string')
    expect(typeof parseInvocation(['export', '--port', 'x'])).toBe('string')
    expect(typeof parseInvocation(['open'])).toBe('string')
  })
})

describe('pickPage', () => {
  const page = (id: string, url: string, type = 'page') => ({id, type, url, webSocketDebuggerUrl: `ws://127.0.0.1/devtools/page/${id}`})

  test('takes the first page when no filter is given', () => {
    expect(pickPage([page('w', 'x', 'service_worker'), page('a', 'about:blank')], undefined)?.id).toBe('a')
  })

  test('takes the first page whose URL contains the filter', () => {
    expect(pickPage([page('a', 'about:blank'), page('b', 'https://preview.example.com/app')], 'example')?.id).toBe('b')
  })

  test('skips a page with no debugger URL', () => {
    expect(pickPage([{id: 'a', type: 'page', url: 'x'}], undefined)).toBeUndefined()
  })
})

describe('settledUrl', () => {
  test('waits for three equal samples that are not blank', () => {
    expect(settledUrl(['about:blank', 'about:blank', 'about:blank'])).toBeUndefined()
    expect(settledUrl(['', '', ''])).toBeUndefined()
    expect(settledUrl(['https://a/', 'https://b/', 'https://b/'])).toBeUndefined()
    expect(settledUrl(['https://a/', 'https://b/', 'https://b/', 'https://b/'])).toBe('https://b/')
  })
})
