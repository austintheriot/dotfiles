import {describe, expect, test} from 'bun:test'
import {chromePolicy, chromePolicyDigest, grantTable, lastAlertedScript, policyDigest, PROMPTS_OFF_SCRIPT, plistXml, plutilKeyPath, REPLAYD_FIELDS, replaydScript, shellQuote, sqlString, tccCheckSql, tccSql, WRITE_CHROME_POLICY_COMMAND, CHROME_POLICY_PATH} from './grants'
import type {TccGrant} from './grants'

const AGENT = '/opt/homebrew/Cellar/tart-guest-agent/0.15.0/bin/tart-guest-agent'

describe('grantTable', () => {
  const grants = grantTable(AGENT)
  const has = (service: string, client: string, target?: string) =>
    grants.some(grant => grant.service === service && (grant.client.kind === 'path' ? grant.client.path : grant.client.id) === client && grant.target === target)

  test('gives the guest agent capture, input, and Full Disk Access', () => {
    for (const service of ['kTCCServiceScreenCapture', 'kTCCServicePostEvent', 'kTCCServiceListenEvent', 'kTCCServiceAccessibility', 'kTCCServiceSystemPolicyAllFiles'])
      expect(has(service, AGENT)).toBe(true)
  })

  test('lets the guest agent send AppleEvents to the apps QA drives', () => {
    for (const target of ['com.apple.systemevents', 'com.apple.finder', 'com.google.Chrome', 'com.github.Electron'])
      expect(has('kTCCServiceAppleEvents', AGENT, target)).toBe(true)
  })

  test('gives Chrome and the desktop app microphone, camera, and screen capture as bundle clients', () => {
    for (const id of ['com.google.Chrome', 'com.github.Electron']) {
      for (const service of ['kTCCServiceMicrophone', 'kTCCServiceCamera', 'kTCCServiceScreenCapture']) expect(has(service, id)).toBe(true)
    }
    expect(grants.filter(grant => grant.client.kind === 'bundle').every(grant => grant.client.kind === 'bundle' && grant.client.id.includes('.'))).toBe(true)
  })

  test('puts microphone grants for the agent in both databases', () => {
    const dbs = grants.filter(grant => grant.service === 'kTCCServiceMicrophone' && grant.client.kind === 'path').map(grant => grant.db)
    expect(dbs.sort()).toEqual(['system', 'user'])
  })

  test('has no duplicate rows', () => {
    const keys = grants.map(grant => JSON.stringify(grant))
    expect(new Set(keys).size).toBe(keys.length)
  })
})

describe('sqlString and tccSql', () => {
  test('doubles single quotes', () => {
    expect(sqlString("it's")).toBe("'it''s'")
  })

  test('quotes a hostile agent path so it stays one value', () => {
    const sql = tccSql(grantTable("/tmp/x'; DROP TABLE access; --"))
    expect(sql.system).toContain("'/tmp/x''; DROP TABLE access; --'")
    expect(sql.system).not.toContain("'/tmp/x'; DROP")
  })

  test('splits rows by database', () => {
    const sql = tccSql(grantTable(AGENT))
    expect(sql.user).toContain('kTCCServiceAppleEvents')
    expect(sql.system).not.toContain('kTCCServiceAppleEvents')
  })

  test('seeds each row from any row macOS wrote, not from one named app', () => {
    const sql = tccSql(grantTable(AGENT))
    expect(sql.system).toContain('INSERT INTO grant_row SELECT * FROM access LIMIT 1;')
    expect(sql.system).not.toContain('osascript')
    expect(sql.user).not.toContain('osascript')
  })
})

const grantClause = (grant: TccGrant) => {
  const client = grant.client.kind === 'path' ? grant.client.path : grant.client.id
  return `(service = '${grant.service}' AND client = '${client}' AND indirect_object_identifier = '${grant.target ?? 'UNUSED'}')`
}

describe('tccCheckSql', () => {
  test('counts granted rows for each database by client, service, and target', () => {
    const grants = grantTable(AGENT)
    const check = tccCheckSql(grants)
    for (const db of ['system', 'user'] as const) {
      expect(check[db].startsWith('SELECT COUNT(*) FROM access WHERE auth_value = 2 AND (')).toBe(true)
      for (const grant of grants.filter(grant => grant.db === db)) expect(check[db]).toContain(grantClause(grant))
    }
  })

  test('names none of the other database grants unless this database also holds them', () => {
    const grants = grantTable(AGENT)
    const check = tccCheckSql(grants)
    for (const db of ['system', 'user'] as const) {
      const other = db === 'system' ? 'user' : 'system'
      const ownClauses = new Set(grants.filter(grant => grant.db === db).map(grantClause))
      for (const grant of grants.filter(grant => grant.db === other)) {
        if (!ownClauses.has(grantClause(grant))) expect(check[db]).not.toContain(grantClause(grant))
      }
    }
  })

  test('quotes a hostile agent path so it stays one value', () => {
    const check = tccCheckSql(grantTable("/tmp/x'; DROP TABLE access; --"))
    expect(check.system).toContain("client = '/tmp/x''; DROP TABLE access; --'")
  })

  test('counts zero for a database with no grants', () => {
    expect(tccCheckSql([]).system).toBe('SELECT COUNT(*) FROM access WHERE 0;')
  })
})

describe('guest scripts', () => {
  const HOSTILE_AGENT = "/Applications/my app.v1/it's/agent"
  const HOSTILE_ENTRY = "'/Applications/my app\\.v1/it'\\''s/agent'"

  test('start with set -e and do not end with a bare true', () => {
    for (const script of [replaydScript(AGENT), PROMPTS_OFF_SCRIPT]) {
      const lines = script.split('\n')
      expect(lines[0]).toBe('set -e')
      expect(lines.at(-1)).not.toBe('true')
    }
  })

  test('allow failure only on the steps that may fail', () => {
    expect(replaydScript(AGENT).split('\n').filter(line => line.includes('|| true'))).toHaveLength(2)
    expect(PROMPTS_OFF_SCRIPT.split('\n').filter(line => line.includes('|| true'))).toHaveLength(2)
  })

  test('replaydScript gives every plutil insert the hostile path as one quoted argument with escaped dots', () => {
    const inserts = replaydScript(HOSTILE_AGENT).split('\n').filter(line => line.startsWith('plutil -insert'))
    expect(inserts).toHaveLength(1 + REPLAYD_FIELDS.length)
    expect(inserts[0]).toBe(`plutil -insert ${HOSTILE_ENTRY} -dictionary "$file"`)
    REPLAYD_FIELDS.forEach((field, index) => {
      expect(inserts[index + 1]).toBe(`plutil -insert '/Applications/my app\\.v1/it'\\''s/agent.${field.key}' -${field.type} ${field.value} "$file"`)
    })
  })

  test('the quoted entry reaches a shell as one argument', () => {
    const result = Bun.spawnSync(['sh', '-c', `printf '[%s]' ${HOSTILE_ENTRY}`])
    expect(result.stdout.toString()).toBe("[/Applications/my app\\.v1/it's/agent]")
  })
})

const ORIGINS = ['https://[*.]example.com']
const CHROME_POLICY = chromePolicy(ORIGINS)

describe('chromePolicy and plistXml', () => {
  test('allows capture and silences prompts', () => {
    expect(CHROME_POLICY).toMatchObject({AudioCaptureAllowed: true, VideoCaptureAllowed: true, DefaultBrowserSettingEnabled: false, PromptForDownloadLocation: false, EnableMediaRouter: false})
  })

  test('renders booleans, integers, and escaped strings', () => {
    const xml = plistXml({On: true, Off: false, Count: 2, Name: 'a<b&c'})
    expect(xml).toContain('<key>On</key><true/>')
    expect(xml).toContain('<key>Off</key><false/>')
    expect(xml).toContain('<key>Count</key><integer>2</integer>')
    expect(xml).toContain('<key>Name</key><string>a&lt;b&amp;c</string>')
    expect(xml.startsWith('<?xml')).toBe(true)
  })

  test('refuses a non-integer number', () => {
    expect(() => plistXml({Ratio: 1.5})).toThrow()
  })

  test('grants the microphone and camera without a prompt to exactly the profile media origins', () => {
    const policy = chromePolicy(['https://meet.google.com', 'https://[*.]example.com'])
    expect(policy.AudioCaptureAllowedUrls).toEqual(['https://meet.google.com', 'https://[*.]example.com'])
    expect(policy.VideoCaptureAllowedUrls).toEqual(policy.AudioCaptureAllowedUrls)
    expect(chromePolicy([]).AudioCaptureAllowedUrls).toEqual([])
  })

  test('the policy digest changes when the media origins change', () => {
    expect(chromePolicyDigest(['https://a.example'])).not.toBe(chromePolicyDigest(['https://b.example']))
  })

  test('opens one blank tab at startup instead of restoring the last session', () => {
    expect(CHROME_POLICY).toMatchObject({RestoreOnStartup: 4, RestoreOnStartupURLs: ['about:blank']})
  })

  test('renders string arrays with each item escaped', () => {
    expect(plistXml({Urls: ['https://a', 'b<c']})).toContain('<key>Urls</key><array><string>https://a</string><string>b&lt;c</string></array>')
  })

  test('the policy digest changes when the policy changes', () => {
    expect(chromePolicyDigest(ORIGINS)).toMatch(/^[0-9a-f]{16}$/)
    expect(policyDigest({...CHROME_POLICY, RestoreOnStartup: 5})).not.toBe(chromePolicyDigest(ORIGINS))
  })
})

describe('replayd entry', () => {
  test('escapes dots in a versioned agent path', () => {
    expect(plutilKeyPath(AGENT, 'kScreenCapturePrivacyHintDate')).toBe('/opt/homebrew/Cellar/tart-guest-agent/0\\.15\\.0/bin/tart-guest-agent.kScreenCapturePrivacyHintDate')
  })

  test('refuses a key with a backslash', () => {
    expect(() => plutilKeyPath('/tmp/a\\b')).toThrow()
  })

  test('sets far-future reminder dates and keeps the 30-day policy', () => {
    expect(REPLAYD_FIELDS).toEqual([
      {key: 'kScreenCaptureApprovalLastAlerted', type: 'date', value: '4000-01-01T00:00:00Z'},
      {key: 'kScreenCapturePrivacyHintDate', type: 'date', value: '4000-01-01T00:00:00Z'},
      {key: 'kScreenCapturePrivacyHintPolicy', type: 'integer', value: '2592000'}
    ])
  })

  test('shellQuote survives a single quote', () => {
    expect(shellQuote("a'b")).toBe(`'a'\\''b'`)
  })
})

describe('lastAlertedScript', () => {
  test('reads the approval date under the escaped agent path', () => {
    const script = lastAlertedScript('/opt/tart/tart-guest-agent.app/agent')
    expect(script).toContain("plutil -extract '/opt/tart/tart-guest-agent\\.app/agent.kScreenCaptureApprovalLastAlerted' raw -o -")
    expect(script).toContain('group.com.apple.replayd/ScreenCaptureApprovals.plist')
  })
})

describe('WRITE_CHROME_POLICY_COMMAND', () => {
  test('writes the managed preferences plist from stdin with sudo tee', () => {
    expect(CHROME_POLICY_PATH).toBe('/Library/Managed Preferences/com.google.Chrome.plist')
    expect(WRITE_CHROME_POLICY_COMMAND).toContain(`sudo tee "${CHROME_POLICY_PATH}"`)
    expect(WRITE_CHROME_POLICY_COMMAND).toContain('sudo mkdir -p "/Library/Managed Preferences"')
  })
})
