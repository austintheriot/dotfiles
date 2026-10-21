#!/usr/bin/env bun
// Edits one Dev QA screen recording: speeds up idle stretches, marks each input, and adds the step caption.
//   bun render.ts <recording.json> <timeline.jsonl> <fonts-dir> <out.mp4> <movie> [<movie>...]
//   Pass one movie per entry in recording.segments, in order. A recording without segments takes one movie.
import {existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {buildSegments, filterGraph, holdPad, holdsBetween, inputWarnings, mapTime, outputCanvas, overlayPlan, screenOf, type Layout, type Size, type TimelineEvent} from './core'
import {bandHtml, chipHtml, FONT_FILES, OVERLAY_SIZES, renderPngs, ringHtml, speedHtml, type Fonts, type OverlayJob} from './overlays'

export type Recording = {step: number | string; caption: string; startedAt: number; captureLag?: number; segments?: {startedAt: number; resumed?: boolean}[]; screen?: Size}

const OUTPUT_FPS = 15
const BAND_HEIGHT = 96
const CRF = 26

async function run(command: string[]): Promise<string> {
  const child = Bun.spawn(command, {stdout: 'pipe', stderr: 'pipe'})
  const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
  if (code !== 0) throw new Error(`${command[0]} exited with ${code}: ${stderr.trim().split('\n').slice(-5).join('\n')}`)
  return stdout
}

async function durationOf(movie: string): Promise<number> {
  const output = await run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', movie])
  const duration = Number(output.trim())
  if (!Number.isFinite(duration) || duration <= 0) throw new Error(`ffprobe gave no duration for ${movie}`)
  return duration
}

async function frameSize(movie: string): Promise<{width: number; height: number}> {
  const output = await run(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=width,height', '-of', 'csv=p=0', movie])
  const [width, height] = output.trim().split(',').map(Number)
  if (!width || !height) throw new Error(`ffprobe gave no frame size for ${movie}`)
  return {width, height}
}

export type CaptureSegment = {movie: string; startedAt: number; resumed?: boolean}

// macOS stops a running screencapture when another app starts a media capture (Chrome joining a
// Meet call), so one step can produce several segments. Holding each segment's last frame until the
// next one starts keeps the joined movie on the wall clock the timeline marks were logged against.
export async function joinSegments(segments: CaptureSegment[], out: string): Promise<void> {
  const durations = await Promise.all(segments.map(segment => durationOf(segment.movie)))
  const holds = holdsBetween(segments.map((segment, index) => ({startedAt: segment.startedAt, duration: durations[index] ?? 0})))
  const first = segments[0]
  if (!first) throw new Error('joinSegments needs at least one segment')
  const {width, height} = await frameSize(first.movie)
  const padded = segments.map((_, index) =>
    `[${index}:v]scale=${width}:${height},setsar=1,fps=30,${holdPad(holds[index] ?? 0, segments[index + 1]?.resumed === true)}[v${index}]`)
  const inputs = segments.map((_, index) => `[v${index}]`).join('')
  await run([
    'ffmpeg', '-loglevel', 'error', '-y',
    ...segments.flatMap(segment => ['-i', segment.movie]),
    '-filter_complex', `${padded.join(';')};${inputs}concat=n=${segments.length}:v=1:a=0[out]`,
    '-map', '[out]', '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '18', '-an',
    out
  ])
}

function relativeEvents(events: TimelineEvent[], recording: Recording, duration: number): TimelineEvent[] {
  const origin = recording.startedAt + (recording.captureLag ?? 0)
  return events
    .map(event => ({...event, at: event.at - origin, endAt: event.endAt - origin}))
    .filter(event => event.at >= 0 && event.at <= duration)
    .sort((left, right) => left.at - right.at)
}

export async function renderVideo(options: {
  movie: string
  recording: Recording
  events: TimelineEvent[]
  fonts: Fonts
  out: string
}): Promise<{duration: number}> {
  const sourceDuration = await durationOf(options.movie)
  const events = relativeEvents(options.events, options.recording, sourceDuration)
  const segments = buildSegments(events, sourceDuration)
  const plan = overlayPlan(events, segments)
  const screen = screenOf(await frameSize(options.movie), options.recording.screen)
  const canvas = outputCanvas(screen)
  const layout: Layout = {...canvas, band: BAND_HEIGHT, ring: OVERLAY_SIZES.ring.width, fps: OUTPUT_FPS, pointScale: canvas.width / screen.width}
  const speeds = [...new Set(plan.speedUps.map(speedUp => speedUp.speed))]

  const workDir = mkdtempSync(join(tmpdir(), 'qa-video-render-'))
  try {
    const jobs: OverlayJob[] = [
      {name: 'band', html: bandHtml(options.recording, options.fonts, layout.width, layout.band), width: layout.width, height: layout.band},
      {name: 'ring', html: ringHtml(options.fonts), ...OVERLAY_SIZES.ring},
      ...plan.chips.map(chip => ({name: `chip-${chip.id}`, html: chipHtml(chip, options.fonts), ...OVERLAY_SIZES.chip})),
      ...speeds.map(speed => ({name: `speed-${speed}`, html: speedHtml(speed, options.fonts), ...OVERLAY_SIZES.speed}))
    ]
    const pngs = await renderPngs(jobs, workDir)
    const order = jobs.map(job => job.name)
    const inputIndex = (name: string) => order.indexOf(name) + 1
    const graph = filterGraph({
      segments,
      plan,
      layout,
      inputs: {
        band: inputIndex('band'),
        ring: inputIndex('ring'),
        chips: Object.fromEntries(plan.chips.map(chip => [chip.id, inputIndex(`chip-${chip.id}`)])),
        speeds: Object.fromEntries(speeds.map(speed => [speed, inputIndex(`speed-${speed}`)]))
      }
    })
    const graphPath = join(workDir, 'graph.txt')
    writeFileSync(graphPath, graph)
    await run([
      'ffmpeg', '-loglevel', 'error', '-y',
      '-i', options.movie,
      ...order.flatMap(name => ['-i', pngs[name] ?? '']),
      '-filter_complex_script', graphPath,
      '-map', '[out]',
      '-r', String(OUTPUT_FPS),
      '-c:v', 'libx264', '-preset', 'veryfast', '-crf', String(CRF), '-pix_fmt', 'yuv420p',
      '-movflags', '+faststart', '-an',
      options.out
    ])
    return {duration: mapTime(segments, sourceDuration)}
  } finally {
    rmSync(workDir, {recursive: true, force: true})
  }
}

function readTimeline(path: string): TimelineEvent[] {
  if (!existsSync(path)) return []
  return readFileSync(path, 'utf8')
    .split('\n')
    .filter(line => line.trim())
    .map(line => JSON.parse(line) as TimelineEvent)
}

if (import.meta.main) {
  const [recordingPath, timelinePath, fontsDir, out, ...movies] = process.argv.slice(2)
  if (!recordingPath || !timelinePath || !fontsDir || !out || movies.length === 0) {
    console.error('usage: bun render.ts <recording.json> <timeline.jsonl> <fonts-dir> <out.mp4> <movie> [<movie>...]')
    process.exit(64)
  }
  const recording = JSON.parse(readFileSync(recordingPath, 'utf8')) as Recording
  const starts = recording.segments?.map(segment => segment.startedAt) ?? [recording.startedAt]
  if (starts.length !== movies.length) {
    console.error(`recording lists ${starts.length} segment(s) but ${movies.length} movie(s) were given`)
    process.exit(64)
  }
  let movie = movies[0] ?? ''
  let joinDir: string | undefined
  if (movies.length > 1) {
    joinDir = mkdtempSync(join(tmpdir(), 'qa-video-join-'))
    movie = join(joinDir, 'joined.mov')
    await joinSegments(
      movies.map((path, index) => ({movie: path, startedAt: starts[index] ?? recording.startedAt, resumed: recording.segments?.[index]?.resumed === true})),
      movie
    )
    console.log(`joined ${movies.length} capture segments; macOS stopped the capture ${movies.length - 1} time(s)`)
  }
  try {
    const events = readTimeline(timelinePath)
    for (const warning of inputWarnings(events)) console.error(`WARNING: step ${recording.step}: ${warning}`)
    const result = await renderVideo({
      movie,
      recording: {...recording, startedAt: starts[0] ?? recording.startedAt},
      events,
      fonts: {
        ui: join(fontsDir, FONT_FILES.ui),
        uiBold: join(fontsDir, FONT_FILES.uiBold),
        mono: join(fontsDir, FONT_FILES.mono)
      },
      out
    })
    console.log(`${out} (${result.duration.toFixed(1)} s)`)
  } finally {
    if (joinDir) rmSync(joinDir, {recursive: true, force: true})
  }
}
