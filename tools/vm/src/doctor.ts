import {existsSync, statSync} from 'node:fs'
import {arch} from 'node:os'
import {BASE_IMAGE, SESSION_FILE} from './config'
import {diagnose, type Check} from './doctor-core'
import {readHostImageState} from './image/base-image'
import {profileStatus} from './profile'
import {run, succeeds} from './proc'

const SOFTNET = '/usr/local/bin/softnet'
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

function softnetFacts(): {owner: string; setuid: boolean} | undefined {
  try {
    const stat = statSync(SOFTNET)
    return {owner: stat.uid === 0 ? 'root' : `uid ${stat.uid}`, setuid: (stat.mode & 0o4000) !== 0}
  } catch {
    return undefined
  }
}

const found = (command: string) => Bun.which(command) !== null

export function doctor(env: Record<string, string | undefined>): Check[] {
  const tart = found('tart') ? run(['tart', '--version']).stdout.trim() : undefined
  return diagnose({
    arch: arch(),
    bunVersion: Bun.version,
    tartVersion: tart || undefined,
    softnet: softnetFacts(),
    ffmpeg: found('ffmpeg'),
    ffprobe: found('ffprobe'),
    chrome: Bun.file(CHROME).size > 0,
    baseImage: tart !== undefined && succeeds(['tart', 'get', BASE_IMAGE]),
    onPath: found('vm'),
    image: readHostImageState(),
    sessionStored: existsSync(SESSION_FILE),
    profile: profileStatus(env)
  }, new Date())
}

export function printChecks(checks: Check[]): number {
  const width = Math.max(...checks.map(check => check.name.length))
  for (const check of checks) {
    console.log(`${check.ok ? 'ok     ' : 'MISSING'}  ${check.name.padEnd(width)}  ${check.detail}${check.ok ? '' : `\n         ${''.padEnd(width)}  fix: ${check.fix}`}`)
  }
  const missing = checks.filter(check => !check.ok).length
  console.log(missing === 0 ? '\nThis Mac is ready for QA VMs.' : `\n${missing} missing. Fix them in order, then run vm doctor again.`)
  return missing === 0 ? 0 : 1
}
