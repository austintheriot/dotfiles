#!/usr/bin/env bun
// Stdio MCP relay in front of Playwright MCP. It sends clicks, typing, hovers, and key presses as
// real macOS input, so the cursor moves on screen the way a person's does. Playwright is used to
// find and measure the target, and to answer each such call with a fresh page snapshot. Input that
// real input cannot reach (inside a frame, a hidden tab) and every other tool still go to Playwright.
// While a Dev QA recording runs, it logs each input with a VM-clock timestamp and screen position.
//   bun proxy.ts <server command...>
import {appendFileSync} from 'node:fs'
import {join} from 'node:path'
import type {TimelineEvent} from './core'
import {bringToFront, osInputEnabled, performOs} from './os-shell'
import {recordingActive} from './recording-state'
import {eventFor, osActionFor, parseProbe, probeTargets, PROBE_FUNCTION, type Probe, type Span} from './timeline'

const PROBE_ID_PREFIX = 'qa-video-probe-'
const PROBE_TIMEOUT_MS = 5000
const OS_TOOLS = new Set(['browser_click', 'browser_type', 'browser_hover', 'browser_press_key'])
const CDP_INPUT_KINDS = new Set(['click', 'type', 'key'])
const SETTLE_MS = 500
const chromePort = Number(process.env['QA_CDP_PORT'] ?? '9333')

type Message = {jsonrpc?: string; id?: string | number; method?: string; params?: {name?: unknown; arguments?: unknown}; result?: unknown}
type PendingInput = {tool: string; args: Record<string, unknown>; probes: Array<Probe | undefined>; at: number}

const dir = process.env['QA_VIDEO_DIR'] ?? '/tmp/pr-review-qa/video'
const command = process.argv.slice(2)
if (command.length === 0) {
  console.error('usage: bun proxy.ts <server command...>')
  process.exit(64)
}

const server = Bun.spawn(command, {stdin: 'pipe', stdout: 'pipe', stderr: 'inherit'})
const probeReplies = new Map<string, (message: Message) => void>()
const pendingInputs = new Map<string | number, PendingInput>()
let probeCount = 0

const now = () => Date.now() / 1000

function sendToServer(message: Message) {
  server.stdin.write(`${JSON.stringify(message)}\n`)
  server.stdin.flush()
}

function resultText(message: Message): string | undefined {
  const result = message.result as {content?: Array<{text?: unknown}>; isError?: boolean} | undefined
  if (!result || result.isError) return undefined
  const text = result.content?.[0]?.text
  return typeof text === 'string' ? text : undefined
}

async function probe(target: string): Promise<Probe | undefined> {
  const id = `${PROBE_ID_PREFIX}${++probeCount}`
  const reply = new Promise<Message | undefined>(resolve => {
    probeReplies.set(id, resolve)
    setTimeout(() => resolve(undefined), PROBE_TIMEOUT_MS)
  })
  sendToServer({jsonrpc: '2.0', id, method: 'tools/call', params: {name: 'browser_evaluate', arguments: {target, function: PROBE_FUNCTION}}})
  const message = await reply
  probeReplies.delete(id)
  const text = message && resultText(message)
  return text === undefined ? undefined : parseProbe(text)
}

async function measure(tool: string, args: Record<string, unknown>): Promise<Array<Probe | undefined>> {
  const probes: Array<Probe | undefined> = []
  for (const target of probeTargets(tool, args)) probes.push(await probe(target))
  return probes
}

function appendEvent(event: TimelineEvent | undefined) {
  if (event) appendFileSync(join(dir, 'timeline.jsonl'), `${JSON.stringify(event)}\n`)
}

async function viaOs(id: string | number, tool: string, args: Record<string, unknown>): Promise<boolean> {
  bringToFront(chromePort)
  const probes = await measure(tool, args)
  const action = osActionFor(tool, args, probes)
  const at = action && performOs(action)
  if (at === undefined) {
    if (recordingActive(dir)) pendingInputs.set(id, {tool, args, probes, at: now()})
    return false
  }
  if (recordingActive(dir)) logInput({tool, args, probes, at}, {at, endAt: now()}, false)
  await Bun.sleep(SETTLE_MS)
  sendToServer({jsonrpc: '2.0', id, method: 'tools/call', params: {name: 'browser_snapshot', arguments: {}}})
  return true
}

async function relayFromClient(message: Message) {
  const tool = message.params?.name
  const args = message.params?.arguments
  const id = message.id
  if (message.method === 'tools/call' && id !== undefined && typeof tool === 'string' && typeof args === 'object' && args !== null) {
    const typedArgs = args as Record<string, unknown>
    if (OS_TOOLS.has(tool) && osInputEnabled()) {
      if (await viaOs(id, tool, typedArgs)) return
    } else if (recordingActive(dir)) {
      pendingInputs.set(id, {tool, args: typedArgs, probes: await measure(tool, typedArgs), at: now()})
    }
  }
  sendToServer(message)
}

function parseMessage(line: string): Message | undefined {
  try {
    const value: unknown = JSON.parse(line)
    return typeof value === 'object' && value !== null ? (value as Message) : undefined
  } catch {
    return undefined
  }
}

function logInput(pending: PendingInput, span: Span, synthetic: boolean) {
  try {
    const event = eventFor(pending.tool, pending.args, pending.probes, span)
    appendEvent(event && synthetic && CDP_INPUT_KINDS.has(event.kind) ? {...event, synthetic: true} : event)
  } catch (error) {
    console.error(`qa-video proxy: could not log ${pending.tool}: ${String(error)}`)
  }
}

function relayFromServer(line: string) {
  const message = parseMessage(line)
  if (typeof message?.id === 'string' && message.id.startsWith(PROBE_ID_PREFIX)) {
    probeReplies.get(message.id)?.(message)
    return
  }
  const pending = message?.id === undefined ? undefined : pendingInputs.get(message.id)
  if (pending && message?.id !== undefined) {
    pendingInputs.delete(message.id)
    logInput(pending, {at: pending.at, endAt: now()}, true)
  }
  process.stdout.write(`${line}\n`)
}

async function relayLineFromClient(line: string) {
  const message = parseMessage(line)
  if (!message) {
    server.stdin.write(`${line}\n`)
    server.stdin.flush()
    return
  }
  try {
    await relayFromClient(message)
  } catch (error) {
    console.error(`qa-video proxy: could not measure the target, relaying unchanged: ${String(error)}`)
    sendToServer(message)
  }
}

async function eachLine(stream: ReadableStream<Uint8Array>, handle: (line: string) => void | Promise<void>) {
  const decoder = new TextDecoder()
  let buffer = ''
  for await (const chunk of stream) {
    buffer += decoder.decode(chunk, {stream: true})
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    for (const line of lines) if (line.trim()) await handle(line)
  }
}

void eachLine(server.stdout, relayFromServer)
await eachLine(Bun.stdin.stream(), relayLineFromClient)
server.stdin.end()
process.exit(await server.exited)
