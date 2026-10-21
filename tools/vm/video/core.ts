export type Point = {x: number; y: number}

export type TimelineEvent = {
  kind: 'click' | 'type' | 'key' | 'navigate' | 'note' | 'pause'
  at: number
  endAt: number
  label: string
  text?: string
  keys?: string[]
  point?: Point
  synthetic?: boolean
}

export type Segment = {from: number; to: number; speed: number; pause?: true}
export type Ring = {x: number; y: number; from: number; to: number}
export type Chip = {id: number; label: string; text: string | undefined; keys?: string[]; from: number; to: number}
export type OverlayPlan = {rings: Ring[]; chips: Chip[]; speedUps: Segment[]}

export type ElementBox = {
  x: number
  y: number
  width: number
  height: number
  screenX: number
  screenY: number
  outerWidth: number
  outerHeight: number
  innerWidth: number
  innerHeight: number
}

const HEAD_KEEP = 2
const LEAD = 1.5
const TRAIL = 2
const TAIL_KEEP = 3
const MIN_SPED_GAP = 4
const MIN_GAP_SPEED = 8
const MAX_GAP_OUTPUT_SECONDS = 2
const PAUSE_OUTPUT_SECONDS = 1.5
const RING_SECONDS = 0.4
const RING_LEAD = 0.1
// The capture starts about 0.4 s after the recorder logs its start, so a click shows on screen that
// much before its logged time. The ring span is measured from the on-screen click.
const CAPTURE_OFFSET = 0.4
const CHIP_MIN_SECONDS = 1
const CHIP_TRAIL = 1.5
const MASK_MAX = 12
const TEXT_MAX = 48
// The chip is at most 472 px wide. After its label, the UI font at 17 px fits about 40 characters.
export const NOTE_MAX = 40
export const textLength = (text: string) => [...text].length

// screencapture writes a frame only when the screen changes. Trimming that source directly drops
// the still time at the start of a segment, so the video runs ahead of its overlays.
const SOURCE_FPS = 30

const round3 = (value: number) => Math.round(value * 1000) / 1000

export function screenPoint(box: ElementBox): Point {
  const left = box.screenX + (box.outerWidth - box.innerWidth) / 2
  const top = box.screenY + (box.outerHeight - box.innerHeight)
  return {x: left + box.x + box.width / 2, y: top + box.y + box.height / 2}
}

const MODIFIER_SYMBOLS: Record<string, string> = {
  Meta: '⌘',
  ControlOrMeta: '⌘',
  Control: '⌃',
  Alt: '⌥',
  Shift: '⇧'
}

const NAMED_KEYS: Record<string, string> = {
  Enter: '↩ Return',
  Escape: 'Esc',
  Tab: '⇥ Tab',
  Backspace: '⌫ Delete',
  Delete: '⌦ Fwd Delete',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  PageUp: 'Page Up',
  PageDown: 'Page Down',
  Home: 'Home',
  End: 'End',
  Space: 'Space',
  ' ': 'Space'
}

export function keyParts(key: string): string[] {
  return key
    .split(/\+(?!$)/)
    .map(part => MODIFIER_SYMBOLS[part] ?? NAMED_KEYS[part] ?? (part.length === 1 ? part.toUpperCase() : part))
}

export const keyLabel = (key: string) => keyParts(key).join(' ')

export type FieldHint = {fieldType: string; autocomplete?: string}

export function displayedText(text: string, field: FieldHint): string {
  const isPassword = field.fieldType === 'password' || (field.autocomplete ?? '').includes('password')
  if (isPassword) return '•'.repeat(Math.min(text.length, MASK_MAX))
  return text.length > TEXT_MAX ? `${text.slice(0, TEXT_MAX - 1)}…` : text
}

export type NameSource = {
  getAttribute(name: string): string | null
  labels?: ArrayLike<{innerText: string}> | null
  innerText?: string
  type?: string
  value?: string
}

// The probes embed this function's source in the page, so it reads nothing outside its own body.
// It reads value only on button-type inputs: on any other field, value is what the person typed.
export function elementName(element: NameSource): string {
  const maxLength = 40
  const firstLabel = element.labels && element.labels.length > 0 ? element.labels[0] : undefined
  const isButton = element.type === 'button' || element.type === 'submit' || element.type === 'reset'
  const name =
    element.getAttribute('aria-label') ||
    (firstLabel ? firstLabel.innerText : '') ||
    element.getAttribute('placeholder') ||
    element.innerText ||
    (isButton ? element.value : '') ||
    element.getAttribute('title') ||
    element.getAttribute('name') ||
    ''
  return (name.trim().split('\n')[0] ?? '').slice(0, maxLength)
}

export const ELEMENT_NAME_SOURCE = elementName.toString()

export function inputWarnings(events: TimelineEvent[]): string[] {
  const inputs = events.filter(event => event.kind !== 'note' && event.kind !== 'pause')
  if (inputs.length === 0) return ['no clicks, keys, or typing were logged for this step, so the video shows none']
  const unplaced = inputs.filter(event => event.kind === 'click' && event.point === undefined).length
  const synthetic = inputs.filter(event => event.synthetic === true).length
  return [
    ...(unplaced === 0
      ? []
      : [unplaced === 1 ? '1 click has no screen position, so the video shows no ring for it' : `${unplaced} clicks have no screen position, so the video shows no ring for them`]),
    ...(synthetic === 0
      ? []
      : [synthetic === 1 ? '1 input went through CDP, so the cursor does not show it' : `${synthetic} inputs went through CDP, so the cursor does not show them`])
  ]
}

type Span = [number, number]

function mergeSpans(spans: Span[]): Span[] {
  const merged: Span[] = []
  for (const span of [...spans].sort((left, right) => left[0] - right[0])) {
    const last = merged.at(-1)
    if (last && span[0] <= last[1]) last[1] = Math.max(last[1], span[1])
    else merged.push([...span])
  }
  return merged
}

function subtractSpans(spans: Span[], holes: Span[]): Span[] {
  return spans
    .flatMap(span =>
      holes.reduce<Span[]>(
        (pieces, [holeFrom, holeTo]) =>
          pieces.flatMap(([from, to]): Span[] => [
            ...(from < holeFrom ? [[from, Math.min(to, holeFrom)] satisfies Span] : []),
            ...(to > holeTo ? [[Math.max(from, holeTo), to] satisfies Span] : [])
          ]),
        [span]
      )
    )
    .filter(([from, to]) => to > from)
}

function keepWindows(events: TimelineEvent[], duration: number): Span[] {
  const clamp = (value: number) => Math.min(Math.max(value, 0), duration)
  const windows: Span[] = [
    [0, HEAD_KEEP],
    ...events.map((event): Span => [event.at - LEAD, event.endAt + TRAIL]),
    [duration - TAIL_KEEP, duration]
  ]
  return mergeSpans(windows.map(([from, to]): Span => [round3(clamp(from)), round3(clamp(to))]))
}

function gapSpeed(length: number): number {
  if (length < MIN_SPED_GAP) return 1
  return Math.max(MIN_GAP_SPEED, Math.ceil(round3(length) / MAX_GAP_OUTPUT_SECONDS))
}

const pauseSpeed = (length: number) => Math.max(1, Math.ceil(round3(length) / PAUSE_OUTPUT_SECONDS))

// A pause wins over the keep window of an input next to it: nothing was captured during a pause.
export function buildSegments(events: TimelineEvent[], duration: number): Segment[] {
  const clamp = (value: number) => round3(Math.min(Math.max(value, 0), duration))
  const pauses = mergeSpans(
    events.filter(event => event.kind === 'pause').map((event): Span => [clamp(event.at), clamp(event.endAt)])
  ).filter(([from, to]) => to > from)
  const kept = subtractSpans(keepWindows(events.filter(event => event.kind !== 'pause'), duration), pauses)
  const fixed: Segment[] = [
    ...kept.map(([from, to]): Segment => ({from, to, speed: 1})),
    ...pauses.map(([from, to]): Segment => ({from, to, speed: pauseSpeed(to - from), pause: true}))
  ].sort((left, right) => left.from - right.from)
  const raw: Segment[] = []
  let cursor = 0
  for (const segment of fixed) {
    if (segment.from > cursor) raw.push({from: cursor, to: segment.from, speed: gapSpeed(segment.from - cursor)})
    raw.push(segment)
    cursor = segment.to
  }
  const segments: Segment[] = []
  for (const segment of raw) {
    const last = segments.at(-1)
    if (last && last.speed === segment.speed && !last.pause && !segment.pause) last.to = segment.to
    else segments.push({...segment})
  }
  return segments.map(segment => ({from: round3(segment.from), to: round3(segment.to), speed: segment.speed, ...(segment.pause ? {pause: true as const} : {})}))
}

export function mapTime(segments: Segment[], at: number): number {
  let output = 0
  for (const segment of segments) {
    if (at >= segment.to) {
      output += (segment.to - segment.from) / segment.speed
    } else {
      if (at > segment.from) output += (at - segment.from) / segment.speed
      break
    }
  }
  return output
}

export function overlayPlan(events: TimelineEvent[], segments: Segment[]): OverlayPlan {
  const rings = events.flatMap(event => {
    if (event.kind !== 'click' || !event.point) return []
    const shown = mapTime(segments, event.at) - CAPTURE_OFFSET
    return [{x: event.point.x, y: event.point.y, from: round3(Math.max(0, shown - RING_LEAD)), to: round3(shown + RING_SECONDS)}]
  })
  const starts = events.map(event => mapTime(segments, event.at))
  const chips = events.map((event, index) => {
    const ownEnd =
      event.kind === 'pause'
        ? mapTime(segments, event.endAt)
        : mapTime(segments, Math.max(event.at + CHIP_MIN_SECONDS, event.endAt + CHIP_TRAIL))
    const nextStart = starts[index + 1]
    return {
      id: index,
      label: event.label,
      text: event.text,
      ...(event.keys ? {keys: event.keys} : {}),
      from: starts[index] ?? 0,
      to: nextStart === undefined ? ownEnd : Math.min(nextStart, ownEnd)
    }
  })
  const speedUps = segments
    .filter(segment => segment.speed > 1 && segment.pause !== true)
    .map(segment => ({from: mapTime(segments, segment.from), to: mapTime(segments, segment.to), speed: segment.speed}))
  return {rings, chips, speedUps}
}

export type Layout = {width: number; height: number; band: number; ring: number; fps: number; pointScale: number}
export type OverlayInputs = {band: number; ring: number; chips: Record<number, number>; speeds: Record<number, number>}

const SPEED_MARGIN = 16

const between = (from: number, to: number) => `enable='between(t,${round3(from)},${round3(to)})'`

function speedOverlays(speedUps: Segment[], speedInputs: Record<number, number>): Array<{input: number; position: string}> {
  return speedUps.flatMap(speedUp => {
    const input = speedInputs[speedUp.speed]
    if (input === undefined) return []
    return [{input, position: `x=main_w-overlay_w-${SPEED_MARGIN}:y=${SPEED_MARGIN}:${between(speedUp.from, speedUp.to)}`}]
  })
}

export function filterGraph(spec: {segments: Segment[]; plan: OverlayPlan; layout: Layout; inputs: OverlayInputs}): string {
  const {segments, plan, layout, inputs} = spec
  const copies = segments.map((_segment, index) => `[c${index}]`).join('')
  const chains = [
    `[0:v]fps=${SOURCE_FPS},split=${segments.length}${copies}`,
    ...segments.map(
      (segment, index) => `[c${index}]trim=start=${segment.from}:end=${segment.to},setpts=(PTS-STARTPTS)/${segment.speed}[s${index}]`
    )
  ]
  const concatInputs = segments.map((_segment, index) => `[s${index}]`).join('')
  chains.push(
    `${concatInputs}concat=n=${segments.length}:v=1:a=0,fps=${layout.fps},scale=${layout.width}:${layout.height},` +
      `pad=width=${layout.width}:height=${layout.height + layout.band}:x=0:y=0[framed]`
  )

  const overlays: Array<{input: number; position: string}> = [
    {input: inputs.band, position: `x=0:y=${layout.height}`},
    ...plan.rings.map(ring => ({
      input: inputs.ring,
      position: `x=${round3(ring.x * layout.pointScale - layout.ring / 2)}:y=${round3(ring.y * layout.pointScale - layout.ring / 2)}:${between(ring.from, ring.to)}`
    })),
    ...plan.chips.flatMap(chip => {
      const input = inputs.chips[chip.id]
      if (input === undefined) return []
      return [{
        input,
        position: `x=main_w-overlay_w:y=${layout.height}:${between(chip.from, chip.to)}`
      }]
    }),
    ...speedOverlays(plan.speedUps, inputs.speeds)
  ]

  let current = 'framed'
  overlays.forEach((overlay, index) => {
    const next = index === overlays.length - 1 ? 'out' : `v${index}`
    chains.push(`[${current}][${overlay.input}:v]overlay=${overlay.position}[${next}]`)
    current = next
  })
  if (current !== 'out') chains.push(`[${current}]null[out]`)
  return chains.join(';')
}

export type TimedSegment = {startedAt: number; duration: number}

export function holdsBetween(segments: TimedSegment[]): number[] {
  return segments.map((segment, index) => {
    const next = segments[index + 1]
    return next ? Math.max(0, next.startedAt - (segment.startedAt + segment.duration)) : 0
  })
}

// Dark frames, not the last captured one, so a pause reads as a pause and not as a frozen screen.
export const PAUSE_FILL = '0x171A20'

export function holdPad(seconds: number, beforeResume: boolean): string {
  const duration = `stop_duration=${seconds.toFixed(3)}`
  return beforeResume ? `tpad=stop_mode=add:color=${PAUSE_FILL}:${duration}` : `tpad=stop_mode=clone:${duration}`
}

export type Size = {width: number; height: number}

const MAX_OUTPUT_WIDTH = 1280
const RETINA_GUESS_WIDTH = 2048

export function outputCanvas(screen: Size): Size {
  const width = Math.min(MAX_OUTPUT_WIDTH, screen.width)
  return {width, height: Math.round((width * screen.height) / screen.width / 2) * 2}
}

// Recordings made before the recorder saved the screen size have only the capture's pixel size.
// macOS captures a Retina screen at twice its point size, and no 1x screen in the VM is this wide.
export function screenOf(capture: Size, saved: Size | undefined): Size {
  if (saved) return saved
  return capture.width >= RETINA_GUESS_WIDTH ? {width: capture.width / 2, height: capture.height / 2} : capture
}

export function parseScreenPoints(output: string): Size | undefined {
  const match = /^(\d+)x(\d+)$/.exec(output.trim())
  return match ? {width: Number(match[1]), height: Number(match[2])} : undefined
}
