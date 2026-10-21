#!/usr/bin/env bun
// Runs inside the QA VM. Controls the full-screen recording of one Dev QA step.
//   bun cli.ts start <step>         caption on stdin; starts screencapture and clears the input timeline
//   bun cli.ts stop                 stops screencapture; prints the recording, timeline, and movie paths as JSON
//   bun cli.ts supervise            started by start; begins a new segment whenever macOS stops the capture
//   bun cli.ts pause                stops the capture until resume; reason on stdin; input is not logged meanwhile
//   bun cli.ts resume               starts a new segment and logs the pause for the editor
//   bun cli.ts mark key <key>       logs a desktop key press, for example Meta+n
//   bun cli.ts mark note            logs a desktop action, text on stdin
import {appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync} from 'node:fs'
import {homedir} from 'node:os'
import {join} from 'node:path'
import {type CaptureFile, firstFrameWritten, mayStartCapture, PAUSE_FILE, RECORDER_REFUSED_EXIT} from './recording-state'
import {keyLabel, keyParts, NOTE_MAX, parseScreenPoints, textLength, type Size, type TimelineEvent} from './core'

const dir = process.env['QA_VIDEO_DIR'] ?? '/tmp/pr-review-qa/video'
const recordingPath = join(dir, 'recording.json')
const timelinePath = join(dir, 'timeline.jsonl')
const SCREENCAPTURE = '/usr/sbin/screencapture'
const CAPTURE_TEMP_DIR = join(homedir(), 'Library/Group Containers/group.com.apple.screencapture/ScreenRecordings')
const FIRST_FRAME_TIMEOUT_MS = 20_000
const stopRequestPath = join(dir, 'stop-requested')
const supervisorPidPath = join(dir, 'supervisor.pid')
const pausePath = join(dir, PAUSE_FILE)
const MARK_SECONDS = 0.3
const SCREEN_POINTS_QUERY = 'ObjC.import("AppKit"); const f = $.NSScreen.mainScreen.frame; `${f.size.width}x${f.size.height}`'
const MAX_SEGMENTS = 50
const QUICK_DEATH_SECONDS = 2
const STOP_LOG_TIMEOUT_MS = 3000
const STOP_LOG_PREDICATE =
  'process == "screencapture" OR process == "replayd" OR subsystem == "com.apple.screencapturekit" OR subsystem == "com.apple.coremedia"'
const stopLogPath = (step: string, index: number) => join(dir, `step-${step}.stop${index}.log`)

type Segment = {movie: string; startedAt: number; resumed?: true}
type Pause = {at: number; reason: string}
type Recording = {step: string; caption: string; startedAt: number; movie: string; segments: Segment[]; screen?: Size; stoppedAt?: number}

const now = () => Date.now() / 1000
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

// The cause of these stops is still unknown, so each one keeps the system log from just before it.
function saveStopLog(step: string, index: number): void {
  const log = Bun.spawnSync({
    cmd: ['/usr/bin/log', 'show', '--last', '20s', '--style', 'compact', '--predicate', STOP_LOG_PREDICATE],
    timeout: STOP_LOG_TIMEOUT_MS,
    killSignal: 'SIGKILL'
  })
  const text = log.exitedDueToTimeout
    ? `log show did not finish within ${STOP_LOG_TIMEOUT_MS} ms, so this log has no system log lines\n`
    : log.exitCode === 0
      ? log.stdout
      : `log show exited ${log.exitCode}: ${log.stderr.toString()}`
  writeFileSync(stopLogPath(step, index), text)
}

function fail(message: string): never {
  console.error(message)
  process.exit(RECORDER_REFUSED_EXIT)
}

function activeRecording(): Recording | undefined {
  if (!existsSync(recordingPath)) return undefined
  const recording = JSON.parse(readFileSync(recordingPath, 'utf8')) as Recording
  return recording.stoppedAt === undefined ? recording : undefined
}

const readRecording = () => JSON.parse(readFileSync(recordingPath, 'utf8')) as Recording
const pidAlive = (pid: number) => Bun.spawnSync(['/bin/kill', '-0', String(pid)]).exitCode === 0
const supervisorAlive = () => existsSync(supervisorPidPath) && pidAlive(Number(readFileSync(supervisorPidPath, 'utf8')))

const paused = () => existsSync(pausePath)
const capturing = () => Bun.spawnSync(['/usr/bin/pgrep', '-x', 'screencapture']).exitCode === 0

async function waitFor(condition: () => boolean, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (condition()) return true
    await sleep(100)
  }
  return condition()
}

async function start(step: string) {
  if (!/^[A-Za-z0-9-]+$/.test(step)) fail(`step must be letters, digits, or dashes, got '${step}'`)
  const running = activeRecording()
  if (running) fail(`step ${running.step} is still recording. Run stop first.`)
  if (capturing()) fail('another screencapture is running in the VM')
  mkdirSync(dir, {recursive: true})
  const caption = (await Bun.stdin.text()).trim()
  writeFileSync(timelinePath, '')
  rmSync(stopRequestPath, {force: true})
  rmSync(pausePath, {force: true})
  for (let index = 0; index < MAX_SEGMENTS; index++) rmSync(stopLogPath(step, index), {force: true})
  const first = (await startSegment(step, 0)) ?? fail('screencapture did not start')
  const screen = parseScreenPoints(Bun.spawnSync(['/usr/bin/osascript', '-l', 'JavaScript', '-e', SCREEN_POINTS_QUERY]).stdout.toString())
  const recording: Recording = {step, caption, startedAt: first.startedAt, movie: first.movie, segments: [first], ...(screen ? {screen} : {})}
  writeFileSync(recordingPath, JSON.stringify(recording))
  Bun.spawnSync(['/bin/sh', '-c', 'nohup "$1" "$2" supervise </dev/null >/dev/null 2>&1 &', 'sh', process.execPath, import.meta.path])
  console.log(`recording step ${step}`)
}

function captureTempFiles(): CaptureFile[] {
  if (!existsSync(CAPTURE_TEMP_DIR)) return []
  return readdirSync(CAPTURE_TEMP_DIR).flatMap(name => {
    try {
      return [{name, size: statSync(join(CAPTURE_TEMP_DIR, name)).size}]
    } catch {
      return []
    }
  })
}

// The segment starts at its first frame, not at process launch. After a pause the first frame
// has arrived seconds after launch, and an action in that gap was missing from the video.
async function startSegment(step: string, index: number): Promise<Segment | undefined> {
  const movie = join(dir, index === 0 ? `step-${step}.mov` : `step-${step}.part${index + 1}.mov`)
  rmSync(movie, {force: true})
  const before = new Set(captureTempFiles().map(file => file.name))
  Bun.spawnSync(['/bin/sh', '-c', `"${SCREENCAPTURE}" -v -k -x "$1" </dev/null >/dev/null 2>&1 &`, 'sh', movie])
  if ((await waitFor(capturing, 5000)) && (await waitFor(() => firstFrameWritten(before, captureTempFiles()), FIRST_FRAME_TIMEOUT_MS))) {
    return {movie, startedAt: now()}
  }
  Bun.spawnSync(['/usr/bin/pkill', '-INT', '-x', 'screencapture'])
  console.error(`screencapture wrote no frame to ${CAPTURE_TEMP_DIR} within ${FIRST_FRAME_TIMEOUT_MS / 1000} s`)
  return undefined
}

const supervisorState = () => ({stopRequested: existsSync(stopRequestPath), paused: paused(), capturing: capturing()})

async function supervise() {
  writeFileSync(supervisorPidPath, String(process.pid))
  let loggedIndex = -1
  try {
    while (!existsSync(stopRequestPath)) {
      await sleep(250)
      if (!mayStartCapture(supervisorState())) continue
      const recording = readRecording()
      if (recording.stoppedAt !== undefined || recording.segments.length >= MAX_SEGMENTS) return
      if (recording.segments.length !== loggedIndex) {
        if (existsSync(stopRequestPath)) return
        saveStopLog(recording.step, recording.segments.length)
        loggedIndex = recording.segments.length
      }
      const last = recording.segments.at(-1)
      if (last && now() - last.startedAt < QUICK_DEATH_SECONDS) await sleep(QUICK_DEATH_SECONDS * 1000)
      if (existsSync(stopRequestPath)) return
      if (!mayStartCapture({...supervisorState(), capturing: false})) continue
      const segment = await startSegment(recording.step, recording.segments.length)
      if (!segment) continue
      if (paused()) {
        Bun.spawnSync(['/usr/bin/pkill', '-INT', '-x', 'screencapture'])
        continue
      }
      if (existsSync(stopRequestPath)) {
        Bun.spawnSync(['/usr/bin/pkill', '-INT', '-x', 'screencapture'])
        return
      }
      writeFileSync(recordingPath, JSON.stringify({...recording, segments: [...recording.segments, segment]}))
    }
  } finally {
    rmSync(supervisorPidPath, {force: true})
  }
}

async function stop() {
  if (!activeRecording()) fail('no recording is running')
  if (paused()) fail('the step is paused. Run resume first, so the video ends on the outcome.')
  writeFileSync(stopRequestPath, '')
  if (!(await waitFor(() => !supervisorAlive(), 10_000))) fail('the capture supervisor did not exit within 10 s')
  Bun.spawnSync(['/usr/bin/pkill', '-INT', '-x', 'screencapture'])
  if (!(await waitFor(() => !capturing(), 30_000))) fail('screencapture did not stop within 30 s')
  const stoppedAt = now()
  const recording = readRecording()
  const segments = recording.segments.filter(segment => existsSync(segment.movie) && statSync(segment.movie).size > 0)
  if (segments.length === 0) fail(`no movie at ${recording.movie}`)
  writeFileSync(recordingPath, JSON.stringify({...recording, segments, stoppedAt}))
  rmSync(stopRequestPath, {force: true})
  if (segments.length > 1) console.error(`macOS stopped the capture ${segments.length - 1} time(s); the recorder resumed each time`)
  const logs = Array.from({length: recording.segments.length}, (_unused, index) => stopLogPath(recording.step, index + 1)).filter(path => existsSync(path))
  console.log(JSON.stringify({recording: recordingPath, timeline: timelinePath, movies: segments.map(segment => segment.movie), logs}))
}

async function mark(kind: string, key: string | undefined) {
  if (!activeRecording()) fail('no recording is running, so the mark was not logged')
  if (paused()) fail('the recording is paused, so the mark was not logged')
  const at = now()
  const span = {at, endAt: at + MARK_SECONDS}
  const note = kind === 'note' ? (await Bun.stdin.text()).trim() : ''
  if (textLength(note) > NOTE_MAX) fail(`the note is ${textLength(note)} characters, and the video chip shows at most ${NOTE_MAX}`)
  const event: TimelineEvent =
    kind === 'key' && key
      ? {kind: 'key', label: 'Pressed', text: keyLabel(key), keys: keyParts(key), ...span}
      : kind === 'note'
        ? {kind: 'note', label: 'Action', text: note, ...span}
        : fail('usage: mark key <key> | mark note (text on stdin)')
  appendFileSync(timelinePath, `${JSON.stringify(event)}\n`)
  console.log(`marked ${event.label}: ${event.text}`)
}

// A retried pause skips the write but still stops the capture, because a failed pause leaves the file behind.
// The supervisor can be mid-restart when the pause lands, so the capture is checked again after its
// quick-death wait. A capture that will not stop means the screen is still recorded.
async function pause() {
  const recording = activeRecording() ?? fail('no recording is running, so there is nothing to pause')
  const reason = (await Bun.stdin.text()).trim() || 'human sign-in'
  if (textLength(reason) > NOTE_MAX) fail(`the reason is ${textLength(reason)} characters, and the video chip shows at most ${NOTE_MAX}`)
  if (!paused()) writeFileSync(pausePath, JSON.stringify({at: now(), reason} satisfies Pause))
  for (let check = 0; check < 2; check++) {
    Bun.spawnSync(['/usr/bin/pkill', '-INT', '-x', 'screencapture'])
    if (!(await waitFor(() => !capturing(), 30_000))) fail('screencapture did not stop within 30 s, so the screen is still recorded. Do not hand off.')
    await sleep((QUICK_DEATH_SECONDS + 1) * 1000)
  }
  if (capturing()) fail('screencapture started again, so the screen is still recorded. Do not hand off.')
  console.log(`paused step ${recording.step}: ${reason}`)
}

async function resume() {
  const recording = activeRecording() ?? fail('no recording is running, so there is nothing to resume')
  if (!paused()) fail(`step ${recording.step} is not paused`)
  const pending = JSON.parse(readFileSync(pausePath, 'utf8')) as Pause
  const segment = await startSegment(recording.step, recording.segments.length)
  if (!segment) fail('screencapture did not start, so the step stays paused')
  const current = readRecording()
  writeFileSync(recordingPath, JSON.stringify({...current, segments: [...current.segments, {...segment, resumed: true}]}))
  const event: TimelineEvent = {kind: 'pause', label: 'Paused', text: pending.reason, at: pending.at, endAt: segment.startedAt}
  appendFileSync(timelinePath, `${JSON.stringify(event)}\n`)
  rmSync(pausePath, {force: true})
  console.log(`resumed step ${recording.step}`)
}

const [command, first, second] = process.argv.slice(2)
switch (command) {
  case 'start':
    await start(first ?? fail('usage: start <step>'))
    break
  case 'stop':
    await stop()
    break
  case 'supervise':
    await supervise()
    break
  case 'pause':
    await pause()
    break
  case 'resume':
    await resume()
    break
  case 'mark':
    await mark(first ?? '', second)
    break
  default:
    fail('usage: start <step> | stop | supervise | pause | resume | mark key <key> | mark note')
}
