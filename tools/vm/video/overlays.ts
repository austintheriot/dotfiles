import {existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import type {Chip} from './core'

export type Fonts = {ui: string; uiBold: string; mono: string}

// vm up copies the profile's fonts into the fonts directory under these names.
export const FONT_FILES = {ui: 'ui.woff2', uiBold: 'ui-bold.woff2', mono: 'mono.woff2'} as const satisfies Fonts
export type Caption = {step: number | string; caption: string}

// Three or more headless Chrome launches at once stall about 20 s each on this host (measured: width 2 never, width 3 and 4 in most batches).
const CHROME_CONCURRENCY = 2
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

const TOKENS = {
  foreground1: '#171A20',
  background1: '#FFFFFF',
  background2: '#F6F6F8',
  background3: '#E8EBF0',
  accent: '#4278ff',
  accentContainer: '#ECF2FF',
  elevationLevel2: '0 3px 10px 0 rgba(23, 26, 32, 0.14)'
}

export const OVERLAY_SIZES = {
  chip: {width: 520, height: 96},
  ring: {width: 64, height: 64},
  speed: {width: 168, height: 48}
}

const escapeHtml = (value: string) =>
  value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

function page(fonts: Fonts, size: {width: number; height: number}, css: string, body: string): string {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face { font-family: 'Overlay UI'; src: url('file://${fonts.ui}') format('woff2'); font-weight: 400; }
@font-face { font-family: 'Overlay UI'; src: url('file://${fonts.uiBold}') format('woff2'); font-weight: 700; }
@font-face { font-family: 'Overlay Mono'; src: url('file://${fonts.mono}') format('woff2'); }
html, body { margin: 0; width: ${size.width}px; height: ${size.height}px; background: transparent; overflow: hidden; }
body { font-family: 'Overlay UI', -apple-system, 'Helvetica Neue', sans-serif; -webkit-font-smoothing: antialiased; }
.mono { font-family: 'Overlay Mono', ui-monospace, Menlo, monospace; }
${css}
</style></head><body>${body}</body></html>`
}

const CHIP_CSS = `
body { box-sizing: border-box; display: flex; align-items: center; justify-content: flex-end; padding-right: 24px; }
.chip { display: flex; align-items: center; gap: 10px; max-width: 472px; padding: 10px 18px;
  border-radius: 999px; background: rgba(23, 26, 32, 0.92); box-shadow: ${TOKENS.elevationLevel2}; color: ${TOKENS.background1}; }
.label { font-weight: 700; font-size: 17px; letter-spacing: 0.2px; white-space: nowrap; }
.text { font-size: 17px; color: ${TOKENS.background3}; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.field { font-size: 15px; padding: 4px 10px; border-radius: 4px; background: ${TOKENS.background2}; color: ${TOKENS.foreground1};
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
kbd { font-family: 'Overlay Mono', ui-monospace, Menlo, monospace; font-size: 16px; min-width: 18px; text-align: center;
  padding: 4px 9px; border-radius: 4px; background: ${TOKENS.background2}; color: ${TOKENS.foreground1};
  box-shadow: inset 0 -2px 0 ${TOKENS.background3}; }
.keys { display: flex; gap: 6px; }
kbd.symbol { font-family: -apple-system, 'Helvetica Neue', sans-serif; font-size: 18px; line-height: 19px; }`

const isSymbolKey = (key: string) => /^[^\p{L}\p{N}\s]$/u.test(key)

function chipBody(chip: Chip): string {
  if (chip.keys && chip.keys.length > 0) {
    return `<span class="keys">${chip.keys.map(key => `<kbd${isSymbolKey(key) ? ' class="symbol"' : ''}>${escapeHtml(key)}</kbd>`).join('')}</span>`
  }
  if (!chip.text) return ''
  const asField = ['Typed', 'Filled', 'Selected', 'Opened'].includes(chip.label)
  return asField ? `<span class="field mono">${escapeHtml(chip.text)}</span>` : `<span class="text">${escapeHtml(chip.text)}</span>`
}

export function chipHtml(chip: Chip, fonts: Fonts): string {
  return page(fonts, OVERLAY_SIZES.chip, CHIP_CSS, `<div class="chip"><span class="label">${escapeHtml(chip.label)}</span>${chipBody(chip)}</div>`)
}

export function bandHtml(caption: Caption, fonts: Fonts, width = 1024, height = 96): string {
  const css = `
body { box-sizing: border-box; display: flex; align-items: center; gap: 16px; padding: 0 24px;
  background: ${TOKENS.background2}; border-top: 1px solid ${TOKENS.background3}; }
.step { flex: none; font-weight: 700; font-size: 16px; padding: 6px 14px; border-radius: 999px;
  background: ${TOKENS.accentContainer}; color: ${TOKENS.accent}; }
.caption { flex: none; width: 440px; font-size: 20px; line-height: 26px; color: ${TOKENS.foreground1};
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
`
  const body = `<span class="step">Step ${escapeHtml(String(caption.step))}</span>` +
    `<span class="caption">${escapeHtml(caption.caption)}</span>`
  return page(fonts, {width, height}, css, body)
}

export function ringHtml(fonts: Fonts): string {
  const css = `
.ring { box-sizing: border-box; width: 64px; height: 64px; border-radius: 50%; border: 4px solid ${TOKENS.accent};
  background: rgba(66, 120, 255, 0.18); box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.9), ${TOKENS.elevationLevel2}; }`
  return page(fonts, OVERLAY_SIZES.ring, css, '<div class="ring"></div>')
}

export function speedHtml(speed: number, fonts: Fonts): string {
  const css = `
body { display: flex; align-items: center; justify-content: flex-end; }
.speed { font-weight: 700; font-size: 16px; padding: 8px 14px; border-radius: 999px; background: ${TOKENS.accent};
  color: ${TOKENS.background1}; box-shadow: ${TOKENS.elevationLevel2}; }`
  return page(fonts, OVERLAY_SIZES.speed, css, `<span class="speed">${speed}× speed</span>`)
}

export type OverlayJob = {name: string; html: string; width: number; height: number}

export const RENDER_TIMEOUT_MS = 30_000

function killGroup(pid: number) {
  try {
    process.kill(-pid, 'SIGKILL')
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    // macOS returns EPERM for a group that holds only unreaped zombies.
    if (code !== 'ESRCH' && code !== 'EPERM') throw error
  }
}

// A file, not a pipe: chrome_crashpad_handler inherits stderr and outlives Chrome, so a pipe never reaches EOF.
// The child leads its own process group, so killing the group removes every helper it left behind.
export async function runInOwnGroup(argv: string[], stderrPath: string, timeoutMs = RENDER_TIMEOUT_MS): Promise<number> {
  const child = Bun.spawn(argv, {stdout: 'ignore', stderr: Bun.file(stderrPath), detached: true})
  const timer = setTimeout(() => killGroup(child.pid), timeoutMs)
  try {
    return await child.exited
  } finally {
    clearTimeout(timer)
    killGroup(child.pid)
  }
}

async function renderPng(job: OverlayJob, outDir: string, profileRoot: string): Promise<readonly [string, string]> {
  const htmlPath = join(outDir, `${job.name}.html`)
  const pngPath = join(outDir, `${job.name}.png`)
  const logPath = join(outDir, `${job.name}.chrome.log`)
  writeFileSync(htmlPath, job.html)
  const code = await runInOwnGroup(
    [
      CHROME,
      '--headless=new',
      `--user-data-dir=${join(profileRoot, job.name)}`,
      '--disable-gpu',
      '--hide-scrollbars',
      '--force-device-scale-factor=1',
      '--default-background-color=00000000',
      '--allow-file-access-from-files',
      '--virtual-time-budget=2000',
      `--window-size=${job.width},${job.height}`,
      `--screenshot=${pngPath}`,
      `file://${htmlPath}`
    ],
    logPath
  )
  // Chrome 155 can crash in teardown ("Teardown watchdog expired", exit 2) after it wrote the screenshot.
  if (existsSync(pngPath) && statSync(pngPath).size > 0) return [job.name, pngPath] as const
  const tail = readFileSync(logPath, 'utf8').trim().split('\n').slice(-3).join('\n')
  throw new Error(`headless Chrome exited with ${code} and wrote no PNG while rendering ${job.name}\n${tail}`)
}

export async function renderPngs(jobs: OverlayJob[], outDir: string, scratchRoot = tmpdir()): Promise<Record<string, string>> {
  const profileRoot = mkdtempSync(join(scratchRoot, 'qa-video-chrome-'))
  try {
    const batches = Array.from({length: Math.ceil(jobs.length / CHROME_CONCURRENCY)}, (_unused, index) =>
      jobs.slice(index * CHROME_CONCURRENCY, (index + 1) * CHROME_CONCURRENCY)
    )
    const rendered: Array<readonly [string, string]> = []
    for (const batch of batches) rendered.push(...(await Promise.all(batch.map(job => renderPng(job, outDir, profileRoot)))))
    return Object.fromEntries(rendered)
  } finally {
    rmSync(profileRoot, {recursive: true, force: true})
  }
}
