import {describe, expect, test} from 'bun:test'
import {classifyGoogleSession, parseSessionRecord, selectSessionCookies, toCookieParam, type CdpCookie} from './session'

const cookie = (name: string, domain: string, extra: Partial<CdpCookie> = {}): CdpCookie => ({
  name,
  value: `value-of-${name}`,
  domain,
  path: '/',
  expires: 2000000000,
  httpOnly: true,
  secure: true,
  ...extra
})

const KEEP_PROXY = [{namePrefix: '_oauth2_proxy', domain: 'example.com'}]

describe('selectSessionCookies', () => {
  test('keeps Google account cookies and the cookies the profile names', () => {
    const kept = selectSessionCookies(
      [
        cookie('SID', '.google.com'),
        cookie('__Host-GAPS', 'accounts.google.com'),
        cookie('_oauth2_proxy', 'staging.example.com'),
        cookie('_oauth2_proxy_1', '.example.com')
      ],
      KEEP_PROXY
    )
    expect(kept.map(kept => kept.name)).toEqual(['SID', '__Host-GAPS', '_oauth2_proxy', '_oauth2_proxy_1'])
  })

  test('keeps only Google account cookies when the profile names none', () => {
    const kept = selectSessionCookies([cookie('SID', '.google.com'), cookie('_oauth2_proxy', 'staging.example.com')], [])
    expect(kept.map(kept => kept.name)).toEqual(['SID'])
  })

  test('drops app session cookies and other sites', () => {
    const kept = selectSessionCookies([cookie('session', 'staging.example.com'), cookie('NID', '.youtube.com'), cookie('SID', 'mail.google.com')], KEEP_PROXY)
    expect(kept).toEqual([])
  })

  test('drops look-alike domains', () => {
    const kept = selectSessionCookies(
      [
        cookie('_oauth2_proxy', 'evilexample.com'),
        cookie('_oauth2_proxy', 'example.com.evil.net'),
        cookie('SID', 'google.com.evil.net'),
        cookie('SID', '.google.com.evil.net')
      ],
      KEEP_PROXY
    )
    expect(kept).toEqual([])
  })
})

describe('toCookieParam', () => {
  test('keeps an expiry in the future', () => {
    expect(toCookieParam(cookie('SID', '.google.com'))).toEqual({
      name: 'SID',
      value: 'value-of-SID',
      domain: '.google.com',
      path: '/',
      expires: 2000000000,
      httpOnly: true,
      secure: true
    })
  })

  test('restores a session cookie without an expiry', () => {
    expect(toCookieParam(cookie('A', '.google.com', {expires: -1, session: true}))).not.toHaveProperty('expires')
    expect(toCookieParam(cookie('B', '.google.com', {expires: -1}))).not.toHaveProperty('expires')
  })

  test('drops read-only fields that setCookies rejects', () => {
    const param = toCookieParam({...cookie('SID', '.google.com', {sameSite: 'Lax', priority: 'High'}), size: 40, session: false} as CdpCookie)
    expect(param).not.toHaveProperty('size')
    expect(param).not.toHaveProperty('session')
    expect(param).toMatchObject({sameSite: 'Lax', priority: 'High'})
  })
})

describe('parseSessionRecord', () => {
  const record = {version: 1 as const, exportedAt: '2026-10-14T10:00:00.000Z', account: 'austin', cookies: [cookie('SID', '.google.com')]}

  test('reads a version 1 record', () => {
    expect(parseSessionRecord(JSON.stringify(record))).toEqual({ok: true, value: record})
  })

  test('names an unknown version', () => {
    expect(parseSessionRecord(JSON.stringify({...record, version: 2}))).toEqual({ok: false, reason: 'unsupported version 2'})
  })

  test('refuses text that is not JSON', () => {
    expect(parseSessionRecord('{')).toEqual({ok: false, reason: 'not JSON'})
  })

  test('refuses a record with a broken cookie, without echoing cookie values', () => {
    const parsed = parseSessionRecord(JSON.stringify({...record, cookies: [{name: 'SID', value: 'secret-value'}]}))
    expect(parsed.ok).toBe(false)
    expect(JSON.stringify(parsed)).not.toContain('secret-value')
  })
})

describe('classifyGoogleSession', () => {
  test('reads a landing on My Account as signed in', () => {
    expect(classifyGoogleSession('https://myaccount.google.com/?utm_source=x')).toBe('signed-in')
  })

  test('reads a sign-in page as signed out', () => {
    expect(classifyGoogleSession('https://accounts.google.com/v3/signin/identifier?continue=x')).toBe('signed-out')
    expect(classifyGoogleSession('https://accounts.google.com/ServiceLogin?service=accountsettings')).toBe('signed-out')
    expect(classifyGoogleSession('https://accounts.google.com/InteractiveLogin')).toBe('signed-out')
  })

  test('reads the signed-out My Account intro page as signed out', () => {
    expect(classifyGoogleSession('https://myaccount.google.com/intro/security')).toBe('signed-out')
  })

  test('reads the signed-out account landing page as signed out', () => {
    expect(classifyGoogleSession('https://www.google.com/account/about/?hl=en-US')).toBe('signed-out')
    expect(classifyGoogleSession('https://www.google.com/account/about')).toBe('signed-out')
  })

  test('reads the account chooser as signed out', () => {
    expect(classifyGoogleSession('https://accounts.google.com/v3/signin/accountchooser?continue=x')).toBe('signed-out')
  })

  test('reads a My Account page whose path starts with intro but is not the intro page as signed in', () => {
    expect(classifyGoogleSession('https://myaccount.google.com/introspect')).toBe('signed-in')
  })

  test('returns unknown for other google.com pages', () => {
    expect(classifyGoogleSession('https://www.google.com/search?q=x')).toBe('unknown')
  })

  test('returns unknown for anything else', () => {
    expect(classifyGoogleSession('about:blank')).toBe('unknown')
    expect(classifyGoogleSession('not a url')).toBe('unknown')
    expect(classifyGoogleSession('http://myaccount.google.com/')).toBe('unknown')
    expect(classifyGoogleSession('https://accounts.google.com/CheckCookie')).toBe('unknown')
  })
})
