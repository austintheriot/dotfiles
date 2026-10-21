import {describe, expect, test} from 'bun:test'
import {mkdtempSync, rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {bandHtml, chipHtml, RENDER_TIMEOUT_MS, renderPngs, runInOwnGroup} from './overlays'

const fonts = {ui: '/fonts/ui.woff2', uiBold: '/fonts/ui-bold.woff2', mono: '/fonts/mono.woff2'}

describe('chipHtml', () => {
  test('shows page text as text, never as markup', () => {
    const html = chipHtml({id: 0, label: 'Click', text: '<img src=x onerror=alert(1)>', from: 0, to: 1}, fonts)
    expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;')
    expect(html).not.toContain('<img')
  })

  test('draws one keycap per key of a chord', () => {
    const html = chipHtml({id: 0, label: 'Pressed', text: '⌘ ⇧ K', keys: ['⌘', '⇧', 'K'], from: 0, to: 1}, fonts)
    expect(html.match(/<kbd/g)?.length).toBe(3)
  })
})

describe('bandHtml', () => {
  test('names the step and its caption', () => {
    const html = bandHtml({step: 2, caption: 'Open a note & draw'}, fonts)
    expect(html).toContain('Step 2')
    expect(html).toContain('Open a note &amp; draw')
  })
})

describe('renderPngs', () => {
  test('leaves no Chrome helper process alive after it returns', async () => {
    const scratch = mkdtempSync(join(tmpdir(), 'qa-overlay-test-'))
    try {
      const jobs = [{name: 'dot', html: '<!doctype html><body style="background:#4278ff">x</body>', width: 32, height: 32}]
      const rendered = await renderPngs(jobs, scratch, scratch)
      expect(Object.keys(rendered)).toEqual(['dot'])
      const survivors = Bun.spawnSync(['/usr/bin/pgrep', '-fl', scratch]).stdout.toString().trim()
      expect(survivors).toBe('')
    } finally {
      rmSync(scratch, {recursive: true, force: true})
    }
  }, RENDER_TIMEOUT_MS + 15_000)
})

describe('runInOwnGroup', () => {
  test('survives killing a group whose members already exited', async () => {
    const attempts = 40
    const failures: unknown[] = []
    for (let attempt = 0; attempt < attempts; attempt++) {
      await runInOwnGroup(['/bin/sh', '-c', 'sleep 5 & sleep 5'], '/dev/null', 150).catch(error => failures.push(error))
    }
    expect(failures).toEqual([])
  }, 30_000)

  test('kills a helper the command leaves behind', async () => {
    const scratch = mkdtempSync(join(tmpdir(), 'qa-group-test-'))
    const marker = `sleep ${3000 + Math.floor(Math.random() * 1000)}`
    try {
      const code = await runInOwnGroup(['/bin/sh', '-c', `${marker} & exit 3`], join(scratch, 'stderr.log'))
      expect(code).toBe(3)
      expect(Bun.spawnSync(['/usr/bin/pgrep', '-f', marker]).stdout.toString().trim()).toBe('')
    } finally {
      Bun.spawnSync(['/usr/bin/pkill', '-f', marker])
      rmSync(scratch, {recursive: true, force: true})
    }
  }, 15_000)
})
