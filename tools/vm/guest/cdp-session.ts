// Runs inside the VM under Bun, with no dependencies. Cookie values move only over stdin and stdout.
type Target = {id: string; type: string; url: string; webSocketDebuggerUrl?: string}
type Action = 'export' | 'import' | 'probe' | 'open'
export type Invocation = {action: Action; port: number; url: string | undefined; target: string | undefined}

const USAGE = 'usage: cdp-session.ts export|import|probe|open [--port N] [--url SUBSTRING] [URL]'
const CALL_TIMEOUT_MS = 10_000
const ACTIONS: readonly string[] = ['export', 'import', 'probe', 'open']

export function parseInvocation(argv: string[]): Invocation | string {
  const [action, ...rest] = argv
  if (action === undefined || !ACTIONS.includes(action)) return USAGE
  let port = 9333
  let url: string | undefined
  let target: string | undefined
  for (let index = 0; index < rest.length; index++) {
    const word = rest[index] ?? ''
    if (word === '--port' || word === '--url') {
      const value = rest[++index]
      if (value === undefined) return `${word} needs a value`
      if (word === '--url') url = value
      else if (/^\d+$/.test(value)) port = Number(value)
      else return `--port must be digits, got '${value}'`
    } else if (action === 'open' && target === undefined) target = word
    else return `unexpected '${word}'. ${USAGE}`
  }
  if (action === 'open' && target === undefined) return 'open needs a URL'
  return {action: action as Action, port, url, target}
}

export const pickPage = (targets: Target[], url: string | undefined): Target | undefined =>
  targets.find(target => target.type === 'page' && target.webSocketDebuggerUrl !== undefined && (url === undefined || target.url.includes(url)))

export function settledUrl(samples: string[], needed = 3): string | undefined {
  const last = samples.slice(-needed)
  const [first] = last
  if (last.length < needed || first === undefined || first === '' || first === 'about:blank') return undefined
  return last.every(sample => sample === first) ? first : undefined
}

class Failure extends Error {
  public constructor(
    message: string,
    public readonly code: number
  ) {
    super(message)
  }
}

async function call(socketUrl: string, method: string, params: object = {}): Promise<Record<string, unknown>> {
  const socket = new WebSocket(socketUrl)
  await new Promise((resolve, reject) => {
    socket.onopen = resolve
    socket.onerror = () => reject(new Failure(`could not connect to ${socketUrl}`, 69))
  })
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Failure(`${method}: timed out`, 69)), CALL_TIMEOUT_MS)
      socket.onclose = () => reject(new Failure(`${method}: Chrome closed the connection`, 69))
      socket.onerror = () => reject(new Failure(`${method}: connection error`, 69))
      socket.onmessage = event => {
        const message = JSON.parse(String(event.data)) as {id?: number; result?: Record<string, unknown>; error?: {message: string}}
        if (message.id !== 1) return
        if (message.error) reject(new Failure(`${method}: ${message.error.message}`, 69))
        else resolve(message.result ?? {})
      }
      socket.send(JSON.stringify({id: 1, method, params}))
    })
  } finally {
    clearTimeout(timer)
    socket.close()
  }
}

async function getJson(port: number, path: string): Promise<unknown> {
  const response = await fetch(`http://127.0.0.1:${port}${path}`).catch(() => undefined)
  if (response?.ok !== true) throw new Failure(`nothing answers CDP on port ${port}`, 69)
  return response.json()
}

async function page(invocation: Invocation): Promise<string> {
  const found = pickPage((await getJson(invocation.port, '/json/list')) as Target[], invocation.url)
  if (found?.webSocketDebuggerUrl === undefined) throw new Failure(`no page on port ${invocation.port}${invocation.url ? ` with '${invocation.url}' in its URL` : ''}`, 66)
  return found.webSocketDebuggerUrl
}

async function browserSocket(port: number): Promise<string> {
  const version = (await getJson(port, '/json/version')) as {webSocketDebuggerUrl?: string}
  if (version.webSocketDebuggerUrl === undefined) throw new Failure(`port ${port} reports no browser endpoint`, 69)
  return version.webSocketDebuggerUrl
}

async function probe(port: number): Promise<string> {
  const browser = await browserSocket(port)
  const created = await call(browser, 'Target.createTarget', {url: 'https://myaccount.google.com', background: true})
  const targetId = String(created['targetId'])
  const samples: string[] = []
  try {
    for (let attempt = 0; attempt < 25; attempt++) {
      await Bun.sleep(1000)
      const target = ((await getJson(port, '/json/list')) as Target[]).find(listed => listed.id === targetId)
      if (target === undefined) throw new Failure('the probe tab disappeared', 69)
      samples.push(target.url)
      const settled = settledUrl(samples)
      if (settled !== undefined) return settled
    }
    return samples.at(-1) ?? ''
  } finally {
    await call(browser, 'Target.closeTarget', {targetId}).catch(() => undefined)
  }
}

async function main(argv: string[]): Promise<number> {
  const invocation = parseInvocation(argv)
  if (typeof invocation === 'string') {
    console.error(invocation)
    return 64
  }
  try {
    if (invocation.action === 'export') {
      const result = await call(await page(invocation), 'Network.getAllCookies')
      process.stdout.write(JSON.stringify(result['cookies'] ?? []))
    } else if (invocation.action === 'import') {
      const cookies = JSON.parse(await Bun.stdin.text()) as unknown
      if (!Array.isArray(cookies)) throw new Failure('stdin must hold a JSON array of cookies', 64)
      await call(await page(invocation), 'Network.setCookies', {cookies})
    } else if (invocation.action === 'probe') {
      console.log(await probe(invocation.port))
    } else {
      await call(await browserSocket(invocation.port), 'Target.createTarget', {url: invocation.target})
    }
    return 0
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    return error instanceof Failure ? error.code : 69
  }
}

if (import.meta.main) process.exit(await main(Bun.argv.slice(2)))
