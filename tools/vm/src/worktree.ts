import {basename, dirname, join} from 'node:path'
import {EXIT} from './config'
import {shellQuote} from './image/grants'
import type {Profile} from './profile'
import {tartExec, VmError, zsh} from './proc'
import {FONT_FILES} from '../video/overlays'

type Fonts = NonNullable<Profile['fonts']>

export const fontCopies = (worktree: string, fontsDir: string, fonts: Fonts): {from: string; to: string}[] =>
  (Object.keys(FONT_FILES) as (keyof Fonts)[]).map(role => ({from: join(worktree, fonts[role]), to: join(fontsDir, FONT_FILES[role])}))

export type GuestDir = '~/wt' | '~/warm'

// Only tracked files go in, so build output and local secrets in the checkout stay on the host.
export function copyCheckout(vm: string, checkout: string, paths: readonly string[], dest: GuestDir): void {
  tartExec(vm, `rm -rf ${dest} && mkdir -p ${dest} && git -C ${dest} init -q`)
  const copied = zsh(`set -o pipefail; git -C "$1" ls-files -z -- "\${@:3}" | tar --null -T - -C "$1" -cf - | tart exec -i "$2" zsh -c "tar -xf - -C ${dest}"`, [
    checkout,
    vm,
    ...paths
  ])
  if (copied.code !== 0) throw new VmError(`copying ${checkout} into ${vm} failed: ${copied.stderr.trim()}`, EXIT.unavailable)
}

export function inputShimScript(driver: string, toolsDir: string): string {
  const file = shellQuote(basename(driver))
  return `cd ~/wt/${shellQuote(dirname(driver))} && [[ -f ${file} && ! -f cdp.real.mjs ]] && mv ${file} cdp.real.mjs && cp ${shellQuote(toolsDir)}/cdp-shim.mjs ${file}`
}
