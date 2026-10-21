#!/usr/bin/env node
// vm up installs this file in the QA VM in place of the worktree's cdp.mjs, and keeps the
// original beside it as cdp.real.mjs. It logs desktop input for the QA video through cdp-log.ts.
// When bun or the logger is missing, it runs the original directly, so the driver never breaks.
// The two paths match VM_BUN and VM_VIDEO_TOOLS in src/config.ts.
import {spawnSync} from 'node:child_process'
import {existsSync} from 'node:fs'
import {fileURLToPath} from 'node:url'

const BUN = '/Users/admin/.bun/bin/bun'
const LOGGER = '/Users/admin/qa-video/cdp-log.ts'
const driver = fileURLToPath(new URL('./cdp.real.mjs', import.meta.url))
const args = process.argv.slice(2)

const logged =
  existsSync(BUN) && existsSync(LOGGER)
    ? spawnSync(BUN, [LOGGER, driver, ...args], {stdio: 'inherit', env: {...process.env, QA_NODE: process.execPath, QA_OS_INPUT: '1'}})
    : undefined
const result = logged && !logged.error ? logged : spawnSync(process.execPath, [driver, ...args], {stdio: 'inherit'})
process.exit(result.status ?? 1)
