import {describe, expect, test} from 'bun:test'
import {parseArgs, type Command} from './args'

const run = async (argv: string[], env: Record<string, string | undefined> = {}): Promise<Command> => {
  const parsed = await parseArgs(argv, env)
  if (parsed.kind !== 'run') throw new Error(`expected a command, got exit ${parsed.code}: ${parsed.stderr}${parsed.stdout}`)
  return parsed.command
}
const exit = async (argv: string[]) => {
  const parsed = await parseArgs(argv, {})
  if (parsed.kind !== 'exit') throw new Error(`expected an exit for ${argv.join(' ')}`)
  return parsed
}

describe('parseArgs', () => {
  test('up takes a PR and a worktree, and shows the VM window by default', async () => {
    expect(await run(['up', '1234', '/tmp/wt'])).toEqual({kind: 'up', pr: 1234, worktree: '/tmp/wt', window: true})
  })

  test('up hides the window with --headless or PR_REVIEW_VM_WINDOW=0', async () => {
    expect(await run(['up', '--headless', '1', '/tmp/wt'])).toMatchObject({window: false})
    expect(await run(['up', '1', '/tmp/wt'], {PR_REVIEW_VM_WINDOW: '0'})).toMatchObject({window: false})
  })

  test('exec joins its words into one command line', async () => {
    expect(await run(['exec', '7', 'cd', 'app', '&&', 'bun', 'install'])).toEqual({kind: 'exec', pr: 7, commandLine: 'cd app && bun install'})
  })

  test('exec passes flags through to the VM command line', async () => {
    expect(await run(['exec', '7', 'ls', '-la'])).toEqual({kind: 'exec', pr: 7, commandLine: 'ls -la'})
  })

  test('pull keeps the VM path and refuses a file name that is unsafe on the host', async () => {
    expect(await run(['pull', '7', '/tmp/pr-review-qa/shot.png'])).toEqual({kind: 'pull', pr: 7, path: '/tmp/pr-review-qa/shot.png', to: undefined})
    expect((await exit(['pull', '7', '/tmp/.hidden'])).stderr).toContain("refusing file name '.hidden'")
  })

  test('pull --to writes into a host directory, for example a unit directory', async () => {
    expect(await run(['pull', '7', '/tmp/shot.png', '--to', 'qa/agents/run/unit'])).toEqual({kind: 'pull', pr: 7, path: '/tmp/shot.png', to: 'qa/agents/run/unit'})
  })

  test('record covers start, stop, rerender, and marks', async () => {
    expect(await run(['record', 'start', '7', '2a', 'Open a note'])).toEqual({kind: 'record-start', pr: 7, step: '2a', caption: 'Open a note'})
    expect(await run(['record', 'stop', '7'])).toEqual({kind: 'record-stop', pr: 7})
    expect(await run(['record', 'rerender', '7', '3'])).toEqual({kind: 'record-rerender', pr: 7, step: '3'})
    expect(await run(['record', 'mark', '7', 'key', 'Meta+n'])).toEqual({kind: 'record-mark', pr: 7, mark: 'key', text: 'Meta+n'})
    expect(await run(['record', 'mark', '7', 'note', 'Dragged the file'])).toEqual({kind: 'record-mark', pr: 7, mark: 'note', text: 'Dragged the file'})
  })

  test('record pause takes an optional reason, and resume takes only the PR', async () => {
    expect(await run(['record', 'pause', '7'])).toEqual({kind: 'record-pause', pr: 7, reason: 'human sign-in'})
    expect(await run(['record', 'pause', '7', 'Google consent'])).toEqual({kind: 'record-pause', pr: 7, reason: 'Google consent'})
    expect(await run(['record', 'resume', '7'])).toEqual({kind: 'record-resume', pr: 7})
  })

  test('record pause refuses a reason longer than the chip shows', async () => {
    expect((await exit(['record', 'pause', '7', 'x'.repeat(41)])).code).toBe(64)
  })

  test('record refuses a step name that is not letters, digits, or dashes', async () => {
    expect((await exit(['record', 'start', '7', '../x', 'caption'])).stderr).toContain('step must be letters, digits, or dashes')
  })

  test('record refuses a mark kind other than key or note', async () => {
    expect((await exit(['record', 'mark', '7', 'click', 'x'])).code).toBe(64)
  })

  test('record refuses mark text longer than the chip shows', async () => {
    const refused = await exit(['record', 'mark', '7', 'note', 'x'.repeat(41)])
    expect(refused.code).toBe(64)
    expect(refused.stderr).toContain('at most 40')
    expect(await run(['record', 'mark', '7', 'note', 'x'.repeat(40)])).toMatchObject({text: 'x'.repeat(40)})
  })

  test('record counts characters, not UTF-16 code units', async () => {
    expect(await run(['record', 'mark', '7', 'note', '𝔸'.repeat(40)])).toMatchObject({text: '𝔸'.repeat(40)})
  })

  test('down, browser, list, and doctor', async () => {
    expect(await run(['down', '7'])).toEqual({kind: 'down', pr: 7})
    expect(await run(['browser', '7'])).toEqual({kind: 'browser', pr: 7})
    expect(await run(['list'])).toEqual({kind: 'list'})
    expect(await run(['ls'])).toEqual({kind: 'list'})
    expect(await run(['doctor'])).toEqual({kind: 'doctor'})
  })

  test('restart takes a PR and refuses extra arguments', async () => {
    expect(await run(['restart', '7'])).toEqual({kind: 'restart', pr: 7, window: true})
    expect((await exit(['restart', '7', '8'])).code).toBe(64)
    expect((await exit(['restart', 'x'])).stderr).toContain("PR number must be digits, got 'x'")
    expect(await exit(['restart', '--help'])).toMatchObject({code: 0, stdout: expect.stringContaining('Softnet')})
    expect(await exit(['restart', '--help'])).toMatchObject({stdout: expect.stringContaining('73 another live session owns the VM')})
  })

  test('restart hides the window with --headless or PR_REVIEW_VM_WINDOW=0, the same rule as up', async () => {
    expect(await run(['restart', '--headless', '7'])).toEqual({kind: 'restart', pr: 7, window: false})
    expect(await run(['restart', '7'], {PR_REVIEW_VM_WINDOW: '0'})).toMatchObject({window: false})
  })

  test('help and version print and exit 0', async () => {
    expect(await exit(['--version'])).toMatchObject({code: 0, stdout: expect.stringMatching(/^\d+\.\d+\.\d+/)})
    expect(await exit(['--help'])).toMatchObject({code: 0, stdout: expect.stringContaining('Usage: vm')})
    expect(await exit(['record', 'start', '--help'])).toMatchObject({code: 0, stdout: expect.stringContaining('<caption>')})
  })

  test('refuses a PR that is not digits', async () => {
    const result = await exit(['down', '7; rm -rf ~'])
    expect(result.code).toBe(64)
    expect(result.stderr).toContain("PR number must be digits, got '7; rm -rf ~'")
  })

  test('refuses missing and extra arguments with exit 64', async () => {
    expect((await exit(['up', '7'])).code).toBe(64)
    expect((await exit(['down', '7', '8'])).code).toBe(64)
    expect((await exit(['exec', '7'])).code).toBe(64)
  })

  test('parses vm image build without a checkout', async () => {
    expect(await run(['image', 'build'])).toEqual({kind: 'image-build', checkout: undefined})
  })

  test('parses vm image build with a checkout', async () => {
    expect(await run(['image', 'build', '--checkout', '/tmp/checkout'])).toEqual({kind: 'image-build', checkout: '/tmp/checkout'})
  })

  test('parses vm image provision', async () => {
    expect(await run(['image', 'provision'])).toEqual({kind: 'image-provision'})
  })

  test('parses vm image grant', async () => {
    expect(await run(['image', 'grant'])).toEqual({kind: 'image-grant'})
  })

  test('parses vm image approve', async () => {
    expect(await run(['image', 'approve'])).toEqual({kind: 'image-approve'})
  })

  test('parses vm image signin', async () => {
    expect(await run(['image', 'signin'])).toEqual({kind: 'image-signin'})
  })

  test('parses vm image warm with a checkout', async () => {
    expect(await run(['image', 'warm', '/tmp/checkout'])).toEqual({kind: 'image-warm', checkout: '/tmp/checkout'})
  })

  test('vm image warm needs a checkout', async () => {
    expect((await exit(['image', 'warm'])).code).toBe(64)
  })

  test('session inject-electron takes a PR and defaults to the Electron debugging port with no url', async () => {
    expect(await run(['session', 'inject-electron', '63933'])).toEqual({kind: 'session-inject-electron', pr: 63933, port: 9224, url: undefined})
  })

  test('session inject-electron takes --port and --url', async () => {
    expect(await run(['session', 'inject-electron', '63933', '--port', '9300', '--url', 'staging'])).toEqual({
      kind: 'session-inject-electron',
      pr: 63933,
      port: 9300,
      url: 'staging'
    })
  })

  test('session inject-electron refuses a port that is not digits', async () => {
    const result = await exit(['session', 'inject-electron', '63933', '--port', 'abc'])
    expect(result.code).toBe(64)
    expect(result.stderr).toContain("port must be 1 to 65535, got 'abc'")
  })

  test('session inject-electron refuses a port outside 1 to 65535', async () => {
    const tooHigh = await exit(['session', 'inject-electron', '63933', '--port', '70000'])
    expect(tooHigh.code).toBe(64)
    expect(tooHigh.stderr).toContain("port must be 1 to 65535, got '70000'")
    const zero = await exit(['session', 'inject-electron', '63933', '--port', '0'])
    expect(zero.code).toBe(64)
    expect(zero.stderr).toContain("port must be 1 to 65535, got '0'")
  })

  test('names an unknown command', async () => {
    const result = await exit(['upp', '7'])
    expect(result.code).toBe(64)
    expect(result.stderr).toContain("unknown command 'upp'")
  })
})
