import {describe, expect, test} from 'bun:test'
import {parseCommand, parseRepo} from './args'
import {HEAD} from './testing/fixtures'

const reasonOf = (argv: string[]) => {
  const parsed = parseCommand(argv)
  return parsed.ok ? 'parsed' : parsed.reason
}

describe('parseCommand', () => {
  test('parses add with a repository and a store root', () => {
    expect(parseCommand(['add', 'review.json', '--repo', 'acme/widgets', '--store', '/tmp/store'])).toEqual({
      ok: true,
      value: {kind: 'add', file: 'review.json', repo: {owner: 'acme', name: 'widgets'}, store: '/tmp/store'}
    })
  })

  test('parses route with its output folder and leaves the defaults unset', () => {
    expect(parseCommand(['route', '--pr', '52', '--head', HEAD, '--out', 'out'])).toEqual({
      ok: true,
      value: {kind: 'route', pr: 52, head: HEAD, out: 'out', repo: undefined, store: undefined}
    })
  })

  test('parses show and create with a config path', () => {
    const expected = {pr: 52, head: HEAD, config: 'c.json', repo: undefined, store: undefined}
    expect(parseCommand(['show', '--pr', '52', '--head', HEAD, '--config', 'c.json'])).toEqual({ok: true, value: {kind: 'show', ...expected}})
    expect(parseCommand(['create', '--pr', '52', '--head', HEAD, '--config', 'c.json'])).toEqual({ok: true, value: {kind: 'create', ...expected}})
  })

  test.each([[[]], [['help']], [['--help']]])('parses %j as help', argv => {
    expect(parseCommand(argv)).toEqual({ok: true, value: {kind: 'help'}})
  })

  test('parses clear with a head and refuses clear with --config', () => {
    expect(parseCommand(['clear', '--pr', '52', '--head', HEAD])).toEqual({ok: true, value: {kind: 'clear', pr: 52, head: HEAD, repo: undefined, store: undefined}})
    expect(reasonOf(['clear', '--pr', '52', '--head', HEAD, '--config', 'c.json'])).toBe('clear does not take --config')
  })

  test('refuses an unknown command, naming it', () => {
    expect(reasonOf(['submit'])).toContain('submit')
  })

  test('refuses add without a file and add with two files', () => {
    expect(reasonOf(['add'])).toContain('one contribution file')
    expect(reasonOf(['add', 'a.json', 'b.json'])).toContain('one contribution file')
  })

  test('refuses a missing --head, naming the option', () => {
    expect(reasonOf(['create', '--pr', '52'])).toContain('--head')
  })

  test('refuses a head that is not a full commit SHA', () => {
    expect(reasonOf(['create', '--pr', '52', '--head', 'abc123'])).toContain('--head')
  })

  test.each(['0', '-3', '1.5', 'x'])('refuses the pull request number %s', pr => {
    expect(reasonOf(['show', '--pr', pr, '--head', HEAD])).toContain('--pr')
  })

  test('refuses route without --out', () => {
    expect(reasonOf(['route', '--pr', '52', '--head', HEAD])).toContain('--out')
  })

  test('refuses an option the command does not take', () => {
    expect(reasonOf(['add', 'a.json', '--pr', '52'])).toContain('--pr')
    expect(reasonOf(['route', '--pr', '52', '--head', HEAD, '--out', 'o', '--config', 'c.json'])).toContain('--config')
  })

  test('refuses an unknown option, naming it', () => {
    expect(reasonOf(['show', '--pr', '52', '--head', HEAD, '--force'])).toContain('--force')
  })

  test('refuses a positional argument after a command that takes none', () => {
    expect(reasonOf(['show', 'extra', '--pr', '52', '--head', HEAD])).toContain('extra')
  })
})

describe('parseRepo', () => {
  test('splits owner and name', () => {
    expect(parseRepo('acme/widgets.js')).toEqual({ok: true, value: {owner: 'acme', name: 'widgets.js'}})
  })

  test.each(['acme', 'acme/widgets/extra', '/widgets', 'acme/', '../x', 'acme/..'])('refuses %s', text => {
    expect(parseRepo(text).ok).toBe(false)
  })
})
