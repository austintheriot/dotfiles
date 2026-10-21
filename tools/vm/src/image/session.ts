import {z} from 'zod'

export const CdpCookie = z
  .object({
    name: z.string(),
    value: z.string(),
    domain: z.string(),
    path: z.string(),
    expires: z.number(),
    httpOnly: z.boolean(),
    secure: z.boolean(),
    session: z.boolean().optional(),
    sameSite: z.enum(['Strict', 'Lax', 'None']).optional(),
    priority: z.enum(['Low', 'Medium', 'High']).optional(),
    sourceScheme: z.enum(['Unset', 'NonSecure', 'Secure']).optional(),
    sourcePort: z.number().optional(),
    partitionKey: z.unknown().optional()
  })
  .passthrough()
export type CdpCookie = z.infer<typeof CdpCookie>

export const SessionRecord = z.object({version: z.literal(1), exportedAt: z.string(), account: z.string(), cookies: z.array(CdpCookie)})
export type SessionRecord = z.infer<typeof SessionRecord>

export type Parsed<Value> = {ok: true; value: Value} | {ok: false; reason: string}

export function parseSessionRecord(text: string): Parsed<SessionRecord> {
  let json: unknown
  try {
    json = JSON.parse(text)
  } catch {
    return {ok: false, reason: 'not JSON'}
  }
  if (typeof json === 'object' && json !== null && 'version' in json && json.version !== 1)
    return {ok: false, reason: `unsupported version ${String(json.version)}`}
  const parsed = SessionRecord.safeParse(json)
  if (parsed.success) return {ok: true, value: parsed.data}
  const issue = parsed.error.issues[0]
  return {ok: false, reason: issue ? `${issue.path.join('.')}: ${issue.code}` : 'invalid record'}
}

const GOOGLE_DOMAINS = new Set(['.google.com', 'accounts.google.com'])

export type KeepCookie = {namePrefix: string; domain: string}

function isWithin(cookieDomain: string, domain: string): boolean {
  const host = cookieDomain.replace(/^\./, '')
  return host === domain || host.endsWith(`.${domain}`)
}

export const selectSessionCookies = (cookies: readonly CdpCookie[], keep: readonly KeepCookie[]): CdpCookie[] =>
  cookies.filter(cookie => GOOGLE_DOMAINS.has(cookie.domain) || keep.some(rule => cookie.name.startsWith(rule.namePrefix) && isWithin(cookie.domain, rule.domain)))

export type CookieParam = Pick<CdpCookie, 'name' | 'value' | 'domain' | 'path' | 'httpOnly' | 'secure'> &
  Partial<Pick<CdpCookie, 'expires' | 'sameSite' | 'priority' | 'sourceScheme' | 'sourcePort' | 'partitionKey'>>

// Network.getAllCookies reports a session cookie with expires -1. Passed back to setCookies, -1 stores it as expired.
export function toCookieParam(cookie: CdpCookie): CookieParam {
  const {name, value, domain, path, httpOnly, secure} = cookie
  const optional = {
    expires: cookie.session === true || cookie.expires < 0 ? undefined : cookie.expires,
    sameSite: cookie.sameSite,
    priority: cookie.priority,
    sourceScheme: cookie.sourceScheme,
    sourcePort: cookie.sourcePort,
    partitionKey: cookie.partitionKey
  }
  const present = Object.fromEntries(Object.entries(optional).filter(([, field]) => field !== undefined))
  return {name, value, domain, path, httpOnly, secure, ...present}
}

export type GoogleSessionState = 'signed-in' | 'signed-out' | 'unknown'

const SIGN_IN_PATHS = [/^\/v3\/signin(\/|$)/, /^\/signin(\/|$)/, /^\/ServiceLogin(\/|$)/, /^\/InteractiveLogin(\/|$)/]

export function classifyGoogleSession(finalUrl: string): GoogleSessionState {
  let url: URL
  try {
    url = new URL(finalUrl)
  } catch {
    return 'unknown'
  }
  if (url.protocol !== 'https:') return 'unknown'
  if (url.hostname === 'myaccount.google.com') return /^\/intro(\/|$)/.test(url.pathname) ? 'signed-out' : 'signed-in'
  if (url.hostname === 'accounts.google.com' && SIGN_IN_PATHS.some(pattern => pattern.test(url.pathname))) return 'signed-out'
  if (url.hostname === 'www.google.com' && /^\/account\/about(\/|$)/.test(url.pathname)) return 'signed-out'
  return 'unknown'
}
