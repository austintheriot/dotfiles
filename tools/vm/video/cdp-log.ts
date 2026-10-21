#!/usr/bin/env bun
// Runs inside the QA VM in front of the repo's cdp.mjs. It sends clicks, keys, and typing as real
// macOS input, so the cursor moves on screen the way a person's does. cdp.mjs is used only to find
// and measure the target. While a Dev QA recording runs, it also logs each input for the editor.
// When real input cannot reach the target (a hidden window, a remote shell), it falls back to the
// original CDP input and marks the logged event as synthetic.
//   bun cdp-log.ts <original cdp.mjs> <cdp.mjs arguments...>
import {appendFileSync} from 'node:fs'
import {join} from 'node:path'
import type {TimelineEvent} from './core'
import {cdpEvent, cdpOsAction, cdpOutput, openExpression, parseCdpCall, parseCdpOutput, parseCdpProbe, probeExpression, type CdpCall, type CdpProbe} from './cdp-timeline'
import {bringToFront, osInputEnabled, performOs} from './os-shell'
import {recordingActive} from './recording-state'

const dir = process.env['QA_VIDEO_DIR'] ?? '/tmp/pr-review-qa/video'
const node = process.env['QA_NODE'] ?? 'node'
const INPUT_COMMANDS = new Set(['click', 'clicktext', 'tap', 'key', 'type', 'setfile', 'open'])
const LOOPBACK = new Set(['127.0.0.1', 'localhost', '::1'])
const [driver, ...args] = process.argv.slice(2)
if (!driver) {
  console.error('usage: bun cdp-log.ts <original cdp.mjs> <cdp.mjs arguments...>')
  process.exit(64)
}

const now = () => Date.now() / 1000

const runDriver = (argv: string[], stdin: 'inherit' | 'ignore') =>
  Bun.spawnSync([node, driver, ...argv], {stdin, stdout: 'pipe', stderr: 'inherit'})

function option(call: CdpCall, name: string): string | undefined {
  const index = call.prefix.indexOf(name)
  return index === -1 ? undefined : call.prefix[index + 1]
}

function portOf(call: CdpCall): number | undefined {
  const optionValues = new Set(['--url', '--host'].map(name => option(call, name)))
  const port = call.prefix.find(arg => /^\d+$/.test(arg) && !optionValues.has(arg))
  return port === undefined ? undefined : Number(port)
}

function measure(call: CdpCall): CdpProbe | undefined {
  const expression = probeExpression(call)
  if (expression === undefined) return undefined
  const result = runDriver([...call.prefix, 'eval', expression], 'ignore')
  return result.exitCode === 0 ? parseCdpProbe(parseCdpOutput(result.stdout.toString())) : undefined
}

function log(event: TimelineEvent | undefined) {
  if (!event || !recordingActive(dir)) return
  try {
    appendFileSync(join(dir, 'timeline.jsonl'), `${JSON.stringify(event)}\n`)
  } catch (error) {
    console.error(`qa-video cdp-log: could not log ${event.label}: ${String(error)}`)
  }
}

function viaCdp(call: CdpCall, probe: CdpProbe | undefined): number {
  const at = now()
  const result = runDriver(args, 'inherit')
  const span = {at, endAt: now()}
  process.stdout.write(result.stdout)
  if (result.exitCode === 0) {
    const event = cdpEvent(call, probe, parseCdpOutput(result.stdout.toString()), span)
    log(event && (event.kind === 'note' ? event : {...event, synthetic: true}))
  }
  return result.exitCode ?? 1
}

function viaOs(call: CdpCall, probe: CdpProbe | undefined): number | undefined {
  const action = cdpOsAction(call, probe)
  if (!action) return undefined
  const at = performOs(action)
  if (at === undefined) return undefined
  const span = {at, endAt: now()}
  const output = cdpOutput(call, probe)
  console.log(JSON.stringify(output, null, 1))
  log(cdpEvent(call, probe, output, span))
  return 0
}

// The URL goes on argv: older cdp.mjs copies have no `eval -` for stdin, so this is the one form every copy takes.
function open(call: CdpCall, original: string): number {
  const expression = openExpression(call.rest[0] ?? '')
  if (expression === undefined) {
    console.log(JSON.stringify({error: 'open needs one http or https URL'}, null, 1))
    return 64
  }
  const at = now()
  const result = Bun.spawnSync([node, original, ...call.prefix, 'eval', expression], {stdout: 'pipe', stderr: 'inherit'})
  process.stdout.write(result.stdout)
  if (result.exitCode === 0) log(cdpEvent(call, undefined, {}, {at, endAt: now()}))
  return result.exitCode ?? 1
}

function run(call: CdpCall): number {
  const port = portOf(call)
  const local = LOOPBACK.has(option(call, '--host') ?? '127.0.0.1') && osInputEnabled()
  if (local && port !== undefined) bringToFront(port)
  const probe = measure(call)
  const output = cdpOutput(call, probe)
  if (probe && ['click', 'clicktext'].includes(call.command) && 'error' in output) {
    console.log(JSON.stringify(output, null, 1))
    return 1
  }
  const os = local ? viaOs(call, probe) : undefined
  if (os !== undefined) return os
  if (call.command !== 'setfile') console.error(`qa-video: sent ${call.command} as CDP input, so the cursor does not show it`)
  return viaCdp(call, probe)
}

const call = parseCdpCall(args)
if (!call || !INPUT_COMMANDS.has(call.command)) {
  process.exit(Bun.spawnSync([node, driver, ...args], {stdio: ['inherit', 'inherit', 'inherit']}).exitCode ?? 1)
}
process.exit(call.command === 'open' ? open(call, driver) : run(call))
