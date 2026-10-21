import {expect, test} from 'bun:test'
import {mkdtempSync, readFileSync, existsSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'

const PAGE = 'data:text/html,<input type=password aria-label=Pass><button>Save it</button>'
const PLAYWRIGHT = ['npx', '-y', '@playwright/mcp@0.0.83', '--headless', '--isolated']

type Message = {id?: unknown; result?: {content?: Array<{text?: string}>; isError?: boolean}}

async function spawnProxy(options: {recording: boolean}) {
  const dir = mkdtempSync(join(tmpdir(), 'qa-video-'))
  if (options.recording) writeFileSync(join(dir, 'recording.json'), JSON.stringify({step: 1, startedAt: 0}))
  const child = Bun.spawn(['bun', join(import.meta.dir, 'proxy.ts'), ...PLAYWRIGHT], {
    stdin: 'pipe',
    stdout: 'pipe',
    stderr: 'ignore',
    cwd: dir,
    env: {...process.env, QA_VIDEO_DIR: dir, QA_OS_INPUT: '0'}
  })
  const received: Message[] = []
  const waiters = new Map<number, {resolve: (message: Message) => void; reject: (error: Error) => void}>()
  void child.exited.then(code => {
    for (const waiter of waiters.values()) waiter.reject(new Error(`proxy exited with code ${code}`))
  })
  void (async () => {
    let buffer = ''
    for await (const chunk of child.stdout) {
      buffer += new TextDecoder().decode(chunk)
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''
      for (const line of lines.filter(Boolean)) {
        const message = JSON.parse(line) as Message
        received.push(message)
        if (typeof message.id === 'number') waiters.get(message.id)?.resolve(message)
      }
    }
  })()
  let nextId = 0
  const request = (method: string, params: object) => {
    const id = ++nextId
    const reply = new Promise<Message>((resolve, reject) => waiters.set(id, {resolve, reject}))
    child.stdin.write(`${JSON.stringify({jsonrpc: '2.0', id, method, params})}\n`)
    child.stdin.flush()
    return reply
  }
  await request('initialize', {protocolVersion: '2025-06-18', capabilities: {}, clientInfo: {name: 'test', version: '0'}})
  child.stdin.write(`${JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'})}\n`)
  return {
    received,
    call: (name: string, args: object) => request('tools/call', {name, arguments: args}),
    timeline: () => {
      const path = join(dir, 'timeline.jsonl')
      if (!existsSync(path)) return []
      return readFileSync(path, 'utf8').trim().split('\n').map(line => JSON.parse(line))
    },
    close: () => child.kill()
  }
}

test('off the VM, sends input through Playwright and flags it as CDP input', async () => {
  const proxy = await spawnProxy({recording: true})
  try {
    await proxy.call('browser_navigate', {url: PAGE})
    const click = await proxy.call('browser_click', {target: 'button'})
    await proxy.call('browser_type', {target: 'input', text: 'hunter2'})

    expect(click.result?.isError).toBeUndefined()
    const [navigate, clicked, typed] = proxy.timeline()
    expect(navigate).toMatchObject({kind: 'navigate', label: 'Opened', text: 'data URL'})
    expect(clicked).toMatchObject({kind: 'click', label: 'Click', text: 'Save it', synthetic: true})
    expect(clicked.point.x).toBeGreaterThan(0)
    expect(clicked.endAt).toBeGreaterThanOrEqual(clicked.at)
    expect(typed).toMatchObject({kind: 'type', text: '•••••••'})
    expect(proxy.received.some(message => String(message.id).startsWith('qa-video'))).toBe(false)
  } finally {
    proxy.close()
  }
}, 90_000)

test('logs nothing when no recording runs', async () => {
  const proxy = await spawnProxy({recording: false})
  try {
    await proxy.call('browser_navigate', {url: PAGE})
    await proxy.call('browser_click', {target: 'button'})
    expect(proxy.timeline()).toEqual([])
  } finally {
    proxy.close()
  }
}, 90_000)
