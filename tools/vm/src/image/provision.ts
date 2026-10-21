import {existsSync, readFileSync, statSync} from 'node:fs'
import {homedir} from 'node:os'
import {EXIT} from '../config'
import {expandHome, type Profile} from '../profile'
import {run, tartExec, VmError} from '../proc'
import {writeMarker} from './base-image'

export const BUN_VERSION = '1.3.14'

const STEPS = [
  'brew update --quiet && brew upgrade --quiet && brew install --quiet node jq git displayplacer cliclick && brew install --quiet --cask google-chrome blackhole-2ch',
  `curl -fsSL https://bun.sh/install | bash -s bun-v${BUN_VERSION}`,
  `grep -q .bun/bin ~/.zprofile 2>/dev/null || echo 'export PATH="$HOME/.bun/bin:$PATH"' >> ~/.zprofile`
]

function guest(vm: string, script: string): void {
  const result = tartExec(vm, script, {stdout: 'inherit', stderr: 'inherit'})
  if (result.code !== 0) throw new VmError(`provisioning ${vm} failed at: ${script.split('\n')[0]}`, EXIT.unavailable)
}

export type ProvisionFile = {guest: string; content: string}

export function readProvisionFiles(files: Profile['provision']['files'], home: string = homedir()): ProvisionFile[] {
  return files.map(file => {
    const host = expandHome(file.host, home)
    if (!existsSync(host) || statSync(host).size === 0) throw new VmError(`no ${host} on the host (the vm profile lists it under provision.files)`, EXIT.noInput)
    return {guest: file.guest, content: readFileSync(host, 'utf8')}
  })
}

// The guest path is relative to the guest home, and only the owner can read it.
function copyIntoGuest(vm: string, file: ProvisionFile): void {
  const copied = run(['tart', 'exec', '-i', vm, 'zsh', '-c', 'umask 077; mkdir -p "$HOME/${1:h}" && cat > "$HOME/$1"', 'zsh', file.guest], {stdin: file.content})
  if (copied.code !== 0) throw new VmError(`copying ${file.guest} into ${vm} failed`, EXIT.unavailable)
}

export function provisionStep(vm: string, files: readonly ProvisionFile[]): void {
  STEPS.forEach(script => guest(vm, script))
  files.forEach(file => copyIntoGuest(vm, file))
  guest(vm, 'date +%Y-%m-%d > ~/.qa-image-provisioned')
  guest(vm, 'set -e; node --version; npm --version; bun --version; jq --version; swiftc --version 2>&1 | head -1; whoami; launchctl managername')
  writeMarker(vm, 'provision', {bun: BUN_VERSION})
}
