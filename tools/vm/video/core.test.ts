import {describe, expect, test} from 'bun:test'
import {
  buildSegments,
  displayedText,
  elementName,
  ELEMENT_NAME_SOURCE,
  filterGraph,
  holdPad,
  holdsBetween,
  inputWarnings,
  keyLabel,
  keyParts,
  mapTime,
  outputCanvas,
  overlayPlan,
  parseScreenPoints,
  screenOf,
  screenPoint,
  type TimelineEvent
} from './core'

const window = {screenX: 0, screenY: 25, outerWidth: 1024, outerHeight: 743, innerWidth: 1024, innerHeight: 640}

describe('screenPoint', () => {
  test('puts the centre of the element below the browser toolbar', () => {
    const point = screenPoint({x: 100, y: 50, width: 200, height: 40, ...window})
    expect(point).toEqual({x: 200, y: 25 + 103 + 70})
  })

  test('accounts for a window moved away from the screen origin', () => {
    const point = screenPoint({x: 0, y: 0, width: 10, height: 10, ...window, screenX: 40, screenY: 60})
    expect(point).toEqual({x: 45, y: 60 + 103 + 5})
  })
})

describe('keyLabel', () => {
  test('uses Mac symbols for modifiers and named keys', () => {
    expect(keyLabel('Meta+Shift+K')).toBe('⌘ ⇧ K')
    expect(keyLabel('Control+a')).toBe('⌃ A')
    expect(keyLabel('Enter')).toBe('↩ Return')
    expect(keyLabel('ArrowLeft')).toBe('←')
    expect(keyLabel('Escape')).toBe('Esc')
  })

  test('keeps a single printable character as typed', () => {
    expect(keyLabel('/')).toBe('/')
  })
})

describe('keyParts', () => {
  test('splits a chord into one keycap per key', () => {
    expect(keyParts('Meta+Shift+K')).toEqual(['⌘', '⇧', 'K'])
    expect(keyParts('Enter')).toEqual(['↩ Return'])
    expect(keyParts('Shift++')).toEqual(['⇧', '+'])
  })
})

describe('displayedText', () => {
  test('masks password fields with one dot per character, up to twelve', () => {
    expect(displayedText('hunter2', {fieldType: 'password'})).toBe('•••••••')
    expect(displayedText('x'.repeat(40), {fieldType: 'password'})).toBe('•'.repeat(12))
  })

  test('masks fields whose autocomplete hint is a password', () => {
    expect(displayedText('secret', {fieldType: 'text', autocomplete: 'current-password'})).toBe('••••••')
  })

  test('shortens long text with an ellipsis', () => {
    expect(displayedText('a'.repeat(80), {fieldType: 'text'})).toBe(`${'a'.repeat(47)}…`)
  })
})

describe('elementName', () => {
  const element = (fields: {attributes?: Record<string, string>; innerText?: string; type?: string; value?: string; labels?: {innerText: string}[]}) => ({
    getAttribute: (name: string) => fields.attributes?.[name] ?? null,
    innerText: fields.innerText ?? '',
    type: fields.type ?? '',
    value: fields.value ?? '',
    labels: fields.labels ?? null
  })

  test('prefers the aria-label, then the label, the placeholder, and the visible text', () => {
    expect(elementName(element({attributes: {'aria-label': 'Close'}, innerText: 'x'}))).toBe('Close')
    expect(elementName(element({labels: [{innerText: 'Email'}], attributes: {placeholder: 'you@example.com'}}))).toBe('Email')
    expect(elementName(element({attributes: {placeholder: 'Search'}}))).toBe('Search')
    expect(elementName(element({innerText: 'Sign up'}))).toBe('Sign up')
  })

  test('names a button, submit, or reset input by its value', () => {
    expect(elementName(element({type: 'button', value: 'Continue'}))).toBe('Continue')
    expect(elementName(element({type: 'submit', value: 'Send link'}))).toBe('Send link')
    expect(elementName(element({type: 'reset', value: 'Clear'}))).toBe('Clear')
  })

  test('never shows what a person typed into a field', () => {
    expect(elementName(element({type: 'email', value: 'austin@example.com', attributes: {name: 'email'}}))).toBe('email')
    expect(elementName(element({type: 'password', value: 'hunter2'}))).toBe('')
    expect(elementName(element({type: 'text', value: 'draft'}))).toBe('')
  })

  test('falls back to the title, then the name attribute', () => {
    expect(elementName(element({attributes: {title: 'More options', name: 'menu'}}))).toBe('More options')
    expect(elementName(element({attributes: {name: 'email'}}))).toBe('email')
  })

  test('keeps the first line and at most 40 characters', () => {
    expect(elementName(element({innerText: '  Coming up\nYour calendar appears empty'}))).toBe('Coming up')
    expect(elementName(element({innerText: 'x'.repeat(60)}))).toBe('x'.repeat(40))
  })

  test('works as page source, the way the probes embed it', () => {
    const fromSource = new Function('element', `return (${ELEMENT_NAME_SOURCE})(element)`) as (element: unknown) => string
    expect(fromSource(element({type: 'submit', value: 'Continue'}))).toBe('Continue')
    expect(fromSource(element({type: 'email', value: 'austin@example.com'}))).toBe('')
  })
})

function click(at: number, endAt = at + 0.3): TimelineEvent {
  return {kind: 'click', at, endAt, label: 'Click', point: {x: 10, y: 10}}
}

describe('buildSegments', () => {
  test('keeps a short recording whole at normal speed', () => {
    expect(buildSegments([click(2)], 6)).toEqual([{from: 0, to: 6, speed: 1}])
  })

  test('speeds up a long gap with no input between two actions', () => {
    const segments = buildSegments([click(2), click(30)], 34)
    expect(segments).toEqual([
      {from: 0, to: 4.3, speed: 1},
      {from: 4.3, to: 28.5, speed: 13},
      {from: 28.5, to: 34, speed: 1}
    ])
  })

  test('keeps the last seconds at normal speed, because they hold the outcome', () => {
    const segments = buildSegments([click(1)], 40)
    expect(segments.at(-1)).toEqual({from: 37, to: 40, speed: 1})
    expect(segments[1]).toEqual({from: 3.3, to: 37, speed: 17})
  })

  test('speeds up a gap at least eight times', () => {
    expect(buildSegments([click(2), click(10)], 13)[1]).toEqual({from: 4.3, to: 8.5, speed: 8})
  })

  test('plays any sped-up gap in at most two seconds, because the agent can think for minutes', () => {
    const [, gap] = buildSegments([click(2), click(302)], 306)
    expect(gap?.speed).toBe(149)
    expect(((gap?.to ?? 0) - (gap?.from ?? 0)) / (gap?.speed ?? 1)).toBeLessThanOrEqual(2)
  })

  test('does not speed up a gap shorter than the minimum', () => {
    expect(buildSegments([click(2), click(8)], 11)).toEqual([{from: 0, to: 11, speed: 1}])
  })
})

describe('mapTime', () => {
  const segments = [
    {from: 0, to: 4, speed: 1},
    {from: 4, to: 20, speed: 8},
    {from: 20, to: 24, speed: 1}
  ]

  test('is the identity inside the first normal-speed segment', () => {
    expect(mapTime(segments, 3)).toBe(3)
  })

  test('compresses time inside a sped-up segment', () => {
    expect(mapTime(segments, 12)).toBe(5)
  })

  test('adds the compressed length to later segments', () => {
    expect(mapTime(segments, 22)).toBe(8)
  })
})

describe('overlayPlan', () => {
  test('shows a ring at each click and a chip for typing, in output time', () => {
    const events: TimelineEvent[] = [
      click(1),
      {kind: 'type', at: 2, endAt: 3, label: 'Typed', text: 'hello'}
    ]
    const plan = overlayPlan(events, [{from: 0, to: 6, speed: 1}])
    expect(plan.rings).toEqual([{x: 10, y: 10, from: 0.5, to: 1}])
    expect(plan.chips).toEqual([
      {id: 0, label: 'Click', text: undefined, from: 1, to: 2},
      {id: 1, label: 'Typed', text: 'hello', from: 2, to: 4.5}
    ])
  })

  test('places the ring 0.1 s before to 0.4 s after the click as it shows on screen', () => {
    const plan = overlayPlan([click(1.5)], [{from: 0, to: 6, speed: 1}])
    expect(plan.rings).toEqual([{x: 10, y: 10, from: 1, to: 1.5}])
  })

  test('marks each sped-up stretch with a speed chip', () => {
    const plan = overlayPlan([click(1), click(30)], buildSegments([click(1), click(30)], 34))
    expect(plan.speedUps).toEqual([{from: 3.3, to: 3.3 + (28.5 - 3.3) / 13, speed: 13}])
  })
})

describe('filterGraph', () => {
  test('trims, retimes, and concatenates each segment before the overlays', () => {
    const graph = filterGraph({
      segments: [
        {from: 0, to: 4, speed: 1},
        {from: 4, to: 20, speed: 8}
      ],
      plan: {rings: [{x: 100, y: 200, from: 1, to: 1.8}], chips: [], speedUps: []},
      layout: {width: 1024, height: 768, band: 96, ring: 64, fps: 15, pointScale: 1},
      inputs: {band: 1, ring: 2, chips: {}, speeds: {}}
    })
    expect(graph).toContain('[0:v]fps=30,split=2[c0][c1]')
    expect(graph).toContain('[c0]trim=start=0:end=4,setpts=(PTS-STARTPTS)/1[s0]')
    expect(graph).toContain('[c1]trim=start=4:end=20,setpts=(PTS-STARTPTS)/8[s1]')
    expect(graph).toContain('[s0][s1]concat=n=2:v=1:a=0')
    expect(graph).toContain("overlay=x=68:y=168:enable='between(t,1,1.8)'")
    expect(graph.endsWith('[out]')).toBe(true)
  })

  test('puts input chips in the caption band, so they never cover the recording', () => {
    const graph = filterGraph({
      segments: [{from: 0, to: 4, speed: 1}],
      plan: {rings: [], chips: [{id: 0, label: 'Click', text: 'Save', from: 1, to: 2}], speedUps: []},
      layout: {width: 1024, height: 768, band: 96, ring: 64, fps: 15, pointScale: 1},
      inputs: {band: 1, ring: 2, chips: {0: 3}, speeds: {}}
    })
    expect(graph).toContain("[3:v]overlay=x=main_w-overlay_w:y=768:enable='between(t,1,2)'")
  })

  test('shows each sped-up stretch with the badge for its own speed', () => {
    const graph = filterGraph({
      segments: [{from: 0, to: 30, speed: 1}],
      plan: {rings: [], chips: [], speedUps: [{from: 2, to: 4, speed: 8}, {from: 6, to: 8, speed: 20}]},
      layout: {width: 1024, height: 768, band: 96, ring: 64, fps: 15, pointScale: 1},
      inputs: {band: 1, ring: 2, chips: {}, speeds: {8: 3, 20: 4}}
    })
    expect(graph).toContain("[3:v]overlay=x=main_w-overlay_w-16:y=16:enable='between(t,2,4)'")
    expect(graph).toContain("[4:v]overlay=x=main_w-overlay_w-16:y=16:enable='between(t,6,8)'")
  })

  test('converts to a constant frame rate before the overlays, because screen recordings only hold frames that changed', () => {
    const graph = filterGraph({
      segments: [{from: 0, to: 4, speed: 1}],
      plan: {rings: [{x: 100, y: 200, from: 1, to: 1.8}], chips: [], speedUps: []},
      layout: {width: 1024, height: 768, band: 96, ring: 64, fps: 15, pointScale: 1},
      inputs: {band: 1, ring: 2, chips: {}, speeds: {}}
    })
    expect(graph).toContain('concat=n=1:v=1:a=0,fps=15,')
    expect(graph.indexOf('fps=15')).toBeLessThan(graph.indexOf('overlay='))
  })
})

describe('holdsBetween', () => {
  test('holds the last frame of each segment until the next one starts', () => {
    const holds = holdsBetween([
      {startedAt: 100, duration: 47.5},
      {startedAt: 149, duration: 30},
      {startedAt: 180, duration: 12}
    ])
    expect(holds).toEqual([1.5, 1, 0])
  })

  test('never holds a negative time when a segment overlaps the next start', () => {
    expect(holdsBetween([{startedAt: 0, duration: 10.2}, {startedAt: 10, duration: 5}])).toEqual([0, 0])
  })

  test('a single segment needs no hold', () => {
    expect(holdsBetween([{startedAt: 5, duration: 60}])).toEqual([0])
  })
})

describe('outputCanvas', () => {
  test('keeps the screen shape, so a 16:10 screen is not squashed into 4:3', () => {
    expect(outputCanvas({width: 1680, height: 1050})).toEqual({width: 1280, height: 800})
  })

  test('renders a 1024x768 screen at its own size, as before', () => {
    expect(outputCanvas({width: 1024, height: 768})).toEqual({width: 1024, height: 768})
  })

  test('rounds the height to an even number, because H.264 needs even dimensions', () => {
    expect(outputCanvas({width: 1366, height: 768})).toEqual({width: 1280, height: 720})
  })
})

describe('screenOf', () => {
  test('uses the screen size the recorder saved', () => {
    expect(screenOf({width: 3360, height: 2100}, {width: 1680, height: 1050})).toEqual({width: 1680, height: 1050})
  })

  test('treats a wide capture with no saved size as a 2x Retina capture', () => {
    expect(screenOf({width: 3360, height: 2100}, undefined)).toEqual({width: 1680, height: 1050})
  })

  test('treats the old 1024x768 VM screen, captured at 2x, as Retina', () => {
    expect(screenOf({width: 2048, height: 1536}, undefined)).toEqual({width: 1024, height: 768})
  })

  test('treats a narrow capture with no saved size as 1x', () => {
    expect(screenOf({width: 1024, height: 768}, undefined)).toEqual({width: 1024, height: 768})
  })
})

describe('parseScreenPoints', () => {
  test('reads the point size that the NSScreen query prints', () => {
    expect(parseScreenPoints('1680x1050\n')).toEqual({width: 1680, height: 1050})
  })

  test('gives nothing when the query printed an error', () => {
    expect(parseScreenPoints('execution error: Error: ReferenceError (-2700)')).toBeUndefined()
  })
})

describe('filterGraph on a scaled canvas', () => {
  test('scales the canvas and places each ring in canvas pixels, not screen points', () => {
    const graph = filterGraph({
      segments: [{from: 0, to: 4, speed: 1}],
      plan: {rings: [{x: 840, y: 525, from: 1, to: 1.8}], chips: [], speedUps: []},
      layout: {width: 1280, height: 800, band: 96, ring: 64, fps: 15, pointScale: 1280 / 1680},
      inputs: {band: 1, ring: 2, chips: {}, speeds: {}}
    })
    expect(graph).toContain('scale=1280:800,pad=width=1280:height=896')
    expect(graph).toContain("overlay=x=608:y=368:enable='between(t,1,1.8)'")
  })
})

describe('inputWarnings', () => {
  const span = {at: 1, endAt: 1.3}

  test('warns when a step logged no input, only notes', () => {
    expect(inputWarnings([{kind: 'note', label: 'Action', text: 'Clicked Trigger', ...span}])).toEqual([
      'no clicks, keys, or typing were logged for this step, so the video shows none'
    ])
    expect(inputWarnings([])).toHaveLength(1)
  })

  test('warns about each click that has no ring', () => {
    const events = [
      {kind: 'click' as const, label: 'Click', text: 'Trigger', point: {x: 1, y: 2}, ...span},
      {kind: 'click' as const, label: 'Click', text: 'Open note', ...span}
    ]
    expect(inputWarnings(events)).toEqual(['1 click has no screen position, so the video shows no ring for it'])
  })

  test('warns about inputs the cursor does not show', () => {
    const events = [
      {kind: 'click' as const, label: 'Click', text: 'Trigger', point: {x: 1, y: 2}, synthetic: true, ...span},
      {kind: 'type' as const, label: 'Filled', text: 'Email: a', synthetic: true, ...span}
    ]
    expect(inputWarnings(events)).toEqual(['2 inputs went through CDP, so the cursor does not show them'])
  })

  test('says nothing when every input is placed', () => {
    const events = [
      {kind: 'click' as const, label: 'Click', text: 'Trigger', point: {x: 1, y: 2}, ...span},
      {kind: 'key' as const, label: 'Pressed', text: '↩ Return', keys: ['↩ Return'], ...span}
    ]
    expect(inputWarnings(events)).toEqual([])
  })
})

const paused = (at: number, endAt: number): TimelineEvent => ({kind: 'pause', label: 'Paused', text: 'human sign-in', at, endAt})

describe('pauses', () => {
  const events = [click(1), paused(5, 65), click(70)]

  test('cuts a pause to at most 1.5 s of output', () => {
    const segments = buildSegments(events, 80)
    expect(segments.find(segment => segment.pause)).toEqual({from: 5, to: 65, speed: 40, pause: true})
    expect(mapTime(segments, 65) - mapTime(segments, 5)).toBe(1.5)
  })

  test('keeps the stretches beside a pause at their own speed', () => {
    expect(buildSegments(events, 80)[0]).toEqual({from: 0, to: 5, speed: 1})
  })

  test('shows no speed badge over a pause', () => {
    const segments = buildSegments(events, 80)
    expect(overlayPlan(events, segments).speedUps.some(speedUp => speedUp.speed === 40)).toBe(false)
  })

  test('shows the Paused chip for exactly the cut stretch', () => {
    const segments = buildSegments(events, 80)
    const chip = overlayPlan(events, segments).chips.find(item => item.label === 'Paused')
    expect(chip).toMatchObject({text: 'human sign-in', from: mapTime(segments, 5), to: mapTime(segments, 65)})
  })

  test('does not count a pause as input', () => {
    expect(inputWarnings([paused(5, 65)])).toEqual(['no clicks, keys, or typing were logged for this step, so the video shows none'])
    expect(inputWarnings([click(1), paused(5, 65)])).toEqual([])
  })

  test('keeps no normal-speed segment over a pause that starts inside an input window', () => {
    const segments = buildSegments([click(4), paused(5, 65)], 80)
    const overlapping = segments.filter(segment => !segment.pause && segment.speed === 1 && segment.from < 65 && segment.to > 5)
    expect(overlapping).toEqual([])
  })

  test('splits an input window in two around a pause inside it', () => {
    const segments = buildSegments([click(20), paused(20.5, 21)], 40)
    const around = segments.filter(segment => segment.from >= 18 && segment.to <= 23)
    expect(around).toEqual([
      {from: 18.5, to: 20.5, speed: 1},
      {from: 20.5, to: 21, speed: 1, pause: true},
      {from: 21, to: 22.3, speed: 1}
    ])
  })

  test('leaves a recording with no pause exactly as before', () => {
    expect(buildSegments([click(1), click(30)], 34)).toEqual([
      {from: 0, to: 3.3, speed: 1},
      {from: 3.3, to: 28.5, speed: 13},
      {from: 28.5, to: 34, speed: 1}
    ])
  })
})

describe('holdPad', () => {
  test('holds the last frame between capture segments', () => {
    expect(holdPad(1.25, false)).toBe('tpad=stop_mode=clone:stop_duration=1.250')
  })

  test('fills the gap before a resumed segment with dark frames', () => {
    expect(holdPad(60, true)).toBe('tpad=stop_mode=add:color=0x171A20:stop_duration=60.000')
  })
})
