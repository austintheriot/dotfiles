import {existsSync, readFileSync} from 'node:fs'
import {join} from 'node:path'
import {EXIT} from '../config'
import type {WarmConfig} from '../profile'
import {run, tartExec, VmError} from '../proc'
import {copyCheckout} from '../worktree'
import {writeMarker, type StepInputs} from './base-image'
import {shellQuote} from './grants'

type Install = WarmConfig['installs'][number]

const sha256 = (file: string) => new Bun.CryptoHasher('sha256').update(readFileSync(file)).digest('hex')

export function dueInstalls(checkout: string, warm: WarmConfig): Install[] {
  const missing = warm.installs.find(install => install.required && !existsSync(join(checkout, install.lockfile)))
  if (missing !== undefined) throw new VmError(`no ${missing.lockfile} in ${checkout}`, EXIT.noInput)
  return warm.installs.filter(install => existsSync(join(checkout, install.lockfile)))
}

export function warmInputs(checkout: string, warm: WarmConfig): StepInputs {
  return Object.fromEntries(dueInstalls(checkout, warm).map(install => [install.lockfile, sha256(join(checkout, install.lockfile))]))
}

// Secrets are read from an interactive zsh because they live in ~/.zshrc. They reach the guest
// only on stdin, for this one install.
export function readSecrets(names: readonly string[]): string[] {
  return names.map(name => {
    const result = run(['zsh', '-ic', `print -r -- "\${${name}:-}"`])
    const value = result.stdout.trim().split('\n').at(-1) ?? ''
    if (value === '') throw new VmError(`${name} is not set in an interactive zsh`, EXIT.unavailable)
    return value
  })
}

export function warmScript(installs: readonly Install[], secretNames: readonly string[]): string {
  return [
    'set -e -o pipefail',
    ...secretNames.map(name => `read -r ${name} && export ${name}`),
    ...installs.map(install => `(cd ~/warm/${shellQuote(install.dir)} && ${install.command} 2>&1 | tail -3)`)
  ].join('\n')
}

export type WarmPlan = {checkout: string; installs: Install[]; inputs: StepInputs; secretNames: string[]; secrets: string[]}

export function planWarm(checkout: string, warm: WarmConfig): WarmPlan {
  const installs = dueInstalls(checkout, warm)
  return {checkout, installs, inputs: warmInputs(checkout, warm), secretNames: warm.secretEnv, secrets: readSecrets(warm.secretEnv)}
}

export function warmStep(vm: string, plan: WarmPlan, paths: readonly string[]): void {
  copyCheckout(vm, plan.checkout, paths, '~/warm')
  const installed = run(['tart', 'exec', '-i', vm, 'zsh', '-lc', warmScript(plan.installs, plan.secretNames)], {
    stdin: plan.secrets.map(secret => `${secret}\n`).join(''),
    stdout: 'inherit',
    stderr: 'inherit'
  })
  if (installed.code !== 0) throw new VmError(`the warm installs in ${vm} failed`, EXIT.unavailable)
  tartExec(vm, 'rm -rf ~/warm && du -sh ~/.bun/install/cache ~/.npm 2>/dev/null', {stdout: 'inherit'})
  writeMarker(vm, 'warm', plan.inputs)
}
