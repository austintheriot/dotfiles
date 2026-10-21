import {afterAll, expect, test} from 'bun:test'
import {existsSync, mkdtempSync, rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {RENDER_TIMEOUT_MS} from './overlays'
import {joinSegments, renderVideo} from './render'

const STARTED_AT = 1_700_000_000
const RENDER_BUDGET_MS = RENDER_TIMEOUT_MS * 4 + 60_000

const scratchDirs: string[] = []

function scratchDir(prefix: string) {
  const dir = mkdtempSync(join(tmpdir(), prefix))
  scratchDirs.push(dir)
  return dir
}

afterAll(() => {
  for (const dir of scratchDirs) rmSync(dir, {recursive: true, force: true})
})

async function probeVideo(path: string) {
  const ffprobe = Bun.spawn(
    ['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=codec_name,width,height:format=duration', '-of', 'json', path],
    {stdout: 'pipe'}
  )
  const parsed = JSON.parse(await new Response(ffprobe.stdout).text())
  return {...parsed.streams[0], duration: Number(parsed.format.duration)}
}

async function makeRecording(dir: string, seconds: number) {
  const movie = join(dir, 'step.mov')
  const ffmpeg = Bun.spawn(['ffmpeg', '-loglevel', 'error', '-f', 'lavfi', '-i', `testsrc2=s=2048x1536:d=${seconds}:r=30`, '-c:v', 'libx264', '-preset', 'ultrafast', movie])
  expect(await ffmpeg.exited).toBe(0)
  return movie
}

test('cuts idle time, adds the caption band, and writes a web-ready H.264 file', async () => {
  const dir = scratchDir('qa-video-render-')
  const movie = await makeRecording(dir, 20)
  const out = join(dir, 'step.mp4')

  const result = await renderVideo({
    movie,
    recording: {step: 1, caption: 'Open the note & check the title', startedAt: STARTED_AT},
    events: [
      {kind: 'click', label: 'Click', text: 'New note', point: {x: 300, y: 200}, at: STARTED_AT + 1, endAt: STARTED_AT + 1.2},
      {kind: 'key', label: 'Pressed', text: '⌘ K', keys: ['⌘', 'K'], at: STARTED_AT + 15, endAt: STARTED_AT + 15.1}
    ],
    fonts: {ui: '/missing/ui.woff2', uiBold: '/missing/bold.woff2', mono: '/missing/mono.woff2'},
    out
  })

  expect(existsSync(out)).toBe(true)
  const video = await probeVideo(out)
  expect(video).toMatchObject({codec_name: 'h264', width: 1024, height: 864})
  expect(video.duration).toBeLessThan(12)
  expect(Math.abs(video.duration - result.duration)).toBeLessThan(0.2)
}, RENDER_BUDGET_MS)

test('joins capture segments into one movie that spans the wall-clock time, holding frames across gaps', async () => {
  const dir = scratchDir('qa-video-join-')
  const first = await makeRecording(dir, 4)
  const secondDir = scratchDir('qa-video-join-')
  const second = await makeRecording(secondDir, 3)
  const out = join(dir, 'joined.mov')

  await joinSegments([{movie: first, startedAt: STARTED_AT}, {movie: second, startedAt: STARTED_AT + 6}], out)

  const video = await probeVideo(out)
  expect(Math.abs(video.duration - 9)).toBeLessThan(0.2)
}, 120_000)

test('keeps still stretches of a capture that only writes frames when the screen changes', async () => {
  const dir = scratchDir('qa-video-vfr-')
  const movie = join(dir, 'still.mov')
  const ffmpeg = Bun.spawn([
    'ffmpeg', '-loglevel', 'error', '-f', 'lavfi', '-i', 'testsrc2=s=1024x768:d=20:r=30',
    '-vf', "select='lt(t,2)+gte(t,18)'", '-fps_mode', 'vfr', '-c:v', 'libx264', '-preset', 'ultrafast', movie
  ])
  expect(await ffmpeg.exited).toBe(0)
  const out = join(dir, 'still.mp4')

  const result = await renderVideo({
    movie,
    recording: {step: 1, caption: 'Still screen', startedAt: STARTED_AT},
    events: [{kind: 'click', label: 'Click', text: 'Save', point: {x: 300, y: 200}, at: STARTED_AT + 10, endAt: STARTED_AT + 10.2}],
    fonts: {ui: '/missing/ui.woff2', uiBold: '/missing/bold.woff2', mono: '/missing/mono.woff2'},
    out
  })

  const video = await probeVideo(out)
  expect(Math.abs(video.duration - result.duration)).toBeLessThan(0.2)
}, RENDER_BUDGET_MS)
