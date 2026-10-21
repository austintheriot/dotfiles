export type TccDb = 'system' | 'user'
export type TccClient = {kind: 'path'; path: string} | {kind: 'bundle'; id: string}
export type TccGrant = {db: TccDb; service: string; client: TccClient; target?: string}

const AGENT_SYSTEM_SERVICES = ['kTCCServiceScreenCapture', 'kTCCServicePostEvent', 'kTCCServiceListenEvent', 'kTCCServiceAccessibility', 'kTCCServiceSystemPolicyAllFiles']
const AGENT_BOTH_DB_SERVICES = ['kTCCServiceAudioCapture', 'kTCCServiceMicrophone']
const APPLE_EVENT_TARGETS = ['com.apple.systemevents', 'com.apple.finder', 'com.google.Chrome', 'com.github.Electron']
const CAPTURE_APPS = ['com.google.Chrome', 'com.github.Electron']

export function grantTable(agentPath: string): TccGrant[] {
  const agent: TccClient = {kind: 'path', path: agentPath}
  const terminal: TccClient = {kind: 'bundle', id: 'com.apple.Terminal'}
  return [
    ...AGENT_SYSTEM_SERVICES.map((service): TccGrant => ({db: 'system', service, client: agent})),
    ...(['system', 'user'] as const).flatMap(db => AGENT_BOTH_DB_SERVICES.map((service): TccGrant => ({db, service, client: agent}))),
    ...APPLE_EVENT_TARGETS.map((target): TccGrant => ({db: 'user', service: 'kTCCServiceAppleEvents', client: agent, target})),
    ...CAPTURE_APPS.flatMap((id): TccGrant[] => [
      {db: 'user', service: 'kTCCServiceMicrophone', client: {kind: 'bundle', id}},
      {db: 'user', service: 'kTCCServiceCamera', client: {kind: 'bundle', id}},
      {db: 'system', service: 'kTCCServiceScreenCapture', client: {kind: 'bundle', id}}
    ]),
    {db: 'system', service: 'kTCCServiceAccessibility', client: terminal},
    {db: 'system', service: 'kTCCServiceSystemPolicyAllFiles', client: terminal},
    {db: 'user', service: 'kTCCServiceAppleEvents', client: terminal, target: 'com.apple.systemevents'}
  ]
}

export const sqlString = (value: string) => `'${value.replaceAll("'", "''")}'`

// The access table's columns differ between macOS versions, so each row starts as a copy of a row
// macOS wrote itself (any row), and only the columns that identify or authorize the grant are changed.
function tccStatement(grant: TccGrant): string {
  const [client, clientType] = grant.client.kind === 'path' ? [grant.client.path, 1] : [grant.client.id, 0]
  return [
    'DELETE FROM grant_row;',
    'INSERT INTO grant_row SELECT * FROM access LIMIT 1;',
    `UPDATE grant_row SET service = ${sqlString(grant.service)}, client = ${sqlString(client)}, client_type = ${clientType}, auth_value = 2, auth_reason = 0,` +
      ` csreq = NULL, indirect_object_identifier_type = 0, indirect_object_identifier = ${sqlString(grant.target ?? 'UNUSED')},` +
      " indirect_object_code_identity = NULL, last_modified = CAST(strftime('%s', 'now') AS INTEGER);",
    'INSERT OR REPLACE INTO access SELECT * FROM grant_row;'
  ].join('\n')
}

export function tccSql(grants: readonly TccGrant[]): Record<TccDb, string> {
  const forDb = (db: TccDb) =>
    ['CREATE TEMP TABLE grant_row AS SELECT * FROM access WHERE 0;', ...grants.filter(grant => grant.db === db).map(tccStatement)].join('\n')
  return {system: forDb('system'), user: forDb('user')}
}

const grantMatch = (grant: TccGrant) => {
  const client = grant.client.kind === 'path' ? grant.client.path : grant.client.id
  return `(service = ${sqlString(grant.service)} AND client = ${sqlString(client)} AND indirect_object_identifier = ${sqlString(grant.target ?? 'UNUSED')})`
}

export function tccCheckSql(grants: readonly TccGrant[]): Record<TccDb, string> {
  const forDb = (db: TccDb) => {
    const matches = grants.filter(grant => grant.db === db).map(grantMatch)
    const where = matches.length === 0 ? 'WHERE 0' : `WHERE auth_value = 2 AND (${matches.join(' OR ')})`
    return `SELECT COUNT(*) FROM access ${where};`
  }
  return {system: forDb('system'), user: forDb('user')}
}

type PlistValue = boolean | number | string | readonly string[]

// Media origins come from the profile: Chrome grants them the microphone and camera without a prompt.
export const chromePolicy = (mediaOrigins: readonly string[]) =>
  ({
    DefaultBrowserSettingEnabled: false,
    PromptForDownloadLocation: false,
    AudioCaptureAllowed: true,
    VideoCaptureAllowed: true,
    AudioCaptureAllowedUrls: mediaOrigins,
    VideoCaptureAllowedUrls: mediaOrigins,
    RestoreOnStartup: 4,
    RestoreOnStartupURLs: ['about:blank'],
    DefaultNotificationsSetting: 2,
    DefaultPopupsSetting: 1,
    MetricsReportingEnabled: false,
    EnableMediaRouter: false
  }) as const satisfies Record<string, PlistValue>

const escapeXml = (text: string) => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')

function plistEntry(key: string, value: PlistValue): string {
  const name = `<key>${escapeXml(key)}</key>`
  if (typeof value === 'boolean') return `${name}${value ? '<true/>' : '<false/>'}`
  if (typeof value === 'number') {
    if (!Number.isInteger(value)) throw new Error(`plist value for ${key} must be an integer, got ${value}`)
    return `${name}<integer>${value}</integer>`
  }
  if (typeof value === 'string') return `${name}<string>${escapeXml(value)}</string>`
  return `${name}<array>${value.map(item => `<string>${escapeXml(item)}</string>`).join('')}</array>`
}

export function plistXml(dict: Record<string, PlistValue>): string {
  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
    '<plist version="1.0"><dict>',
    ...Object.entries(dict).map(([key, value]) => plistEntry(key, value)),
    '</dict></plist>',
    ''
  ].join('\n')
}

export const policyDigest = (policy: Record<string, PlistValue>): string =>
  new Bun.CryptoHasher('sha256').update(plistXml(policy)).digest('hex').slice(0, 16)

export const chromePolicyDigest = (mediaOrigins: readonly string[]): string => policyDigest(chromePolicy(mediaOrigins))

export const CHROME_POLICY_PATH = '/Library/Managed Preferences/com.google.Chrome.plist'

// The guest command reads the plist on stdin, so it never carries the policy text in argv.
export const WRITE_CHROME_POLICY_COMMAND = `sudo mkdir -p "/Library/Managed Preferences" && sudo tee "${CHROME_POLICY_PATH}" >/dev/null`

export function plutilKeyPath(...keys: string[]): string {
  for (const key of keys) if (key.includes('\\')) throw new Error(`plutil key path cannot hold a backslash: ${key}`)
  return keys.map(key => key.replaceAll('.', '\\.')).join('.')
}

export const shellQuote = (value: string) => `'${value.replaceAll("'", `'\\''`)}'`

const FAR_FUTURE = '4000-01-01T00:00:00Z'
export const REPLAYD_FIELDS = [
  {key: 'kScreenCaptureApprovalLastAlerted', type: 'date', value: FAR_FUTURE},
  {key: 'kScreenCapturePrivacyHintDate', type: 'date', value: FAR_FUTURE},
  {key: 'kScreenCapturePrivacyHintPolicy', type: 'integer', value: '2592000'}
] as const satisfies readonly {key: string; type: 'date' | 'integer'; value: string}[]

// replayd keys each approval by the requester's full path, and a dot in that path is a key separator for plutil.
export function replaydScript(agentPath: string): string {
  const entry = shellQuote(plutilKeyPath(agentPath))
  return [
    'set -e',
    'file="$HOME/Library/Group Containers/group.com.apple.replayd/ScreenCaptureApprovals.plist"',
    'mkdir -p "${file:h}"',
    '[[ -f $file ]] || plutil -create binary1 "$file"',
    `plutil -remove ${entry} "$file" 2>/dev/null || true`,
    `plutil -insert ${entry} -dictionary "$file"`,
    ...REPLAYD_FIELDS.map(field => `plutil -insert ${shellQuote(plutilKeyPath(agentPath, field.key))} -${field.type} ${field.value} "$file"`),
    'killall replayd 2>/dev/null || true'
  ].join('\n')
}

export const LAST_ALERTED_KEY = 'kScreenCaptureApprovalLastAlerted'

export function lastAlertedScript(agent: string): string {
  return `plutil -extract ${shellQuote(plutilKeyPath(agent, LAST_ALERTED_KEY))} raw -o - "$HOME/Library/Group Containers/group.com.apple.replayd/ScreenCaptureApprovals.plist"`
}

// Tips and Notification Center banners land in recordings. Login relaunch reopens apps over the window under test.
export const PROMPTS_OFF_SCRIPT = [
  'set -e',
  'launchctl disable gui/$(id -u)/com.apple.tipsd',
  'launchctl bootout gui/$(id -u)/com.apple.tipsd 2>/dev/null || true',
  'launchctl disable gui/$(id -u)/com.apple.notificationcenterui',
  'launchctl bootout gui/$(id -u)/com.apple.notificationcenterui 2>/dev/null || true',
  'defaults write com.apple.loginwindow TALLogoutSavesState -bool false',
  'defaults write com.apple.loginwindow LoginwindowLaunchesRelaunchApps -bool false',
  'defaults -currentHost write com.apple.loginwindow TALLogoutSavesState -bool false',
  'defaults -currentHost write com.apple.loginwindow LoginwindowLaunchesRelaunchApps -bool false',
  'rm -rf ~/Library/Saved\\ Application\\ State/com.google.Chrome.savedState'
].join('\n')
