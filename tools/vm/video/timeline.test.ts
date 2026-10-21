import {describe, expect, test} from 'bun:test'
import {eventFor, osActionFor, parseProbe, probeTargets, shortUrl, type Probe} from './timeline'

function probe(overrides: Partial<Probe> = {}): Probe {
  return {
    x: 100,
    y: 50,
    width: 200,
    height: 40,
    screenX: 0,
    screenY: 25,
    outerWidth: 1024,
    outerHeight: 743,
    innerWidth: 1024,
    innerHeight: 640,
    fieldType: 'button',
    autocomplete: '',
    name: 'Save it',
    inFrame: false,
    visible: true,
    ...overrides
  }
}

describe('probeTargets', () => {
  test('measures the one element a click or a type acts on', () => {
    expect(probeTargets('browser_click', {target: 'e12'})).toEqual(['e12'])
    expect(probeTargets('browser_type', {target: 'e3', text: 'hi'})).toEqual(['e3'])
  })

  test('measures every field of a form fill', () => {
    const fields = [
      {target: 'e1', name: 'Email', type: 'textbox', value: 'a@b.co'},
      {target: 'e2', name: 'Remember', type: 'checkbox', value: 'true'}
    ]
    expect(probeTargets('browser_fill_form', {fields})).toEqual(['e1', 'e2'])
  })

  test('measures nothing for a key press or a navigation', () => {
    expect(probeTargets('browser_press_key', {key: 'Enter'})).toEqual([])
    expect(probeTargets('browser_navigate', {url: 'https://example.com'})).toEqual([])
  })
})

describe('parseProbe', () => {
  test('reads the JSON between the Result heading and the next heading', () => {
    const text = '### Result\n{\n  "x": 161,\n  "name": "a \\"quoted\\" name"\n}\n### Ran Playwright code\n```js\n```'
    expect(parseProbe(text)).toEqual({x: 161, name: 'a "quoted" name'} as unknown as Probe)
  })

  test('returns undefined for an error or a result with no JSON', () => {
    expect(parseProbe('### Error\nError: Ref e99 not found')).toBeUndefined()
    expect(parseProbe('### Result\nnot json\n### Ran')).toBeUndefined()
  })
})

describe('eventFor', () => {
  const span = {at: 10, endAt: 10.4}

  test('a click carries the element name and its centre on screen', () => {
    expect(eventFor('browser_click', {target: 'e1'}, [probe()], span)).toEqual({
      kind: 'click',
      label: 'Click',
      text: 'Save it',
      point: {x: 200, y: 198},
      ...span
    })
  })

  test('a click names its modifiers and its kind', () => {
    const event = eventFor('browser_click', {target: 'e1', doubleClick: true, modifiers: ['Meta', 'Shift']}, [probe()], span)
    expect(event?.label).toBe('Double-click')
    expect(event?.text).toBe('⌘ ⇧ Save it')
  })

  test('a click inside an iframe has no ring, because its position is frame-relative', () => {
    expect(eventFor('browser_click', {target: 'e1'}, [probe({inFrame: true})], span)?.point).toBeUndefined()
  })

  test('typing into a password field shows dots', () => {
    const event = eventFor('browser_type', {target: 'e1', text: 'hunter2'}, [probe({fieldType: 'password'})], span)
    expect(event).toMatchObject({kind: 'type', label: 'Typed', text: '•••••••'})
  })

  test('typing shows dots when the field could not be measured', () => {
    expect(eventFor('browser_type', {target: 'e1', text: 'hunter2'}, [undefined], span)?.text).toBe('•••••••')
  })

  test('typing with submit shows the Return key', () => {
    expect(eventFor('browser_type', {target: 'e1', text: 'notes', submit: true}, [probe({fieldType: 'text'})], span)?.text).toBe(
      'notes ↩'
    )
  })

  test('a key press shows the key with Mac symbols', () => {
    expect(eventFor('browser_press_key', {key: 'Meta+k'}, [], span)).toEqual({kind: 'key', label: 'Pressed', text: '⌘ K', keys: ['⌘', 'K'], ...span})
  })

  test('a form fill lists each field and masks password fields by type or by name', () => {
    const fields = [
      {target: 'e1', name: 'Email', type: 'textbox', value: 'a@b.co'},
      {target: 'e2', name: 'Password', type: 'textbox', value: 'secret'},
      {target: 'e3', name: 'PIN', type: 'textbox', value: '1234'},
      {target: 'e4', name: 'Remember me', type: 'checkbox', value: 'true'}
    ]
    const probes = [probe({fieldType: 'email'}), probe({fieldType: 'text'}), probe({fieldType: 'password'}), probe({fieldType: 'checkbox'})]
    expect(eventFor('browser_fill_form', {fields}, probes, span)?.text).toBe(
      'Email: a@b.co · Password: •••••• · PIN: •••• · Remember me: on'
    )
  })

  test('a navigation shows the host and path, never the query string', () => {
    expect(eventFor('browser_navigate', {url: 'https://preview.example.com/notes?token=abc'}, [], span)).toEqual({
      kind: 'navigate',
      label: 'Opened',
      text: 'preview.example.com/notes',
      ...span
    })
  })

  test('a hover or a snapshot is not an input event', () => {
    expect(eventFor('browser_hover', {target: 'e1'}, [probe()], span)).toBeUndefined()
    expect(eventFor('browser_snapshot', {}, [], span)).toBeUndefined()
  })
})

describe('osActionFor', () => {
  const centre = {x: 200, y: 198}

  test('clicks the element centre on the screen', () => {
    expect(osActionFor('browser_click', {target: 'e1'}, [probe()])).toEqual({kind: 'click', point: centre, button: 'left', double: false, modifiers: []})
  })

  test('keeps double-click, right button, and modifiers', () => {
    const args = {target: 'e1', doubleClick: true, button: 'right', modifiers: ['Shift']}
    expect(osActionFor('browser_click', args, [probe()])).toEqual({kind: 'click', point: centre, button: 'right', double: true, modifiers: ['Shift']})
  })

  test('replaces the text of the typed field, and submits when asked', () => {
    expect(osActionFor('browser_type', {target: 'e3', text: 'hi', submit: true}, [probe()])).toEqual({kind: 'type', text: 'hi', submit: true, field: centre})
  })

  test('moves the pointer for a hover', () => {
    expect(osActionFor('browser_hover', {target: 'e1'}, [probe()])).toEqual({kind: 'move', point: centre})
  })

  test('presses a key without measuring anything', () => {
    expect(osActionFor('browser_press_key', {key: 'Meta+k'}, [])).toEqual({kind: 'key', key: 'Meta+k'})
  })

  test('leaves the input to Playwright when real input cannot reach the element', () => {
    expect(osActionFor('browser_click', {target: 'e1'}, [undefined])).toBeUndefined()
    expect(osActionFor('browser_click', {target: 'e1'}, [probe({inFrame: true})])).toBeUndefined()
    expect(osActionFor('browser_click', {target: 'e1'}, [probe({visible: false})])).toBeUndefined()
    expect(osActionFor('browser_click', {target: 'e1', button: 'middle'}, [probe()])).toBeUndefined()
  })

  test('leaves form fills, selects, and navigation to Playwright', () => {
    expect(osActionFor('browser_fill_form', {fields: []}, [])).toBeUndefined()
    expect(osActionFor('browser_select_option', {target: 'e1', values: ['a']}, [probe()])).toBeUndefined()
    expect(osActionFor('browser_navigate', {url: 'https://example.com'}, [])).toBeUndefined()
  })
})

describe('probeTargets for a hover', () => {
  test('measures the hovered element', () => {
    expect(probeTargets('browser_hover', {target: 'e9'})).toEqual(['e9'])
  })
})

describe('shortUrl', () => {
  test('shows the host and path, never the query', () => {
    expect(shortUrl('https://preview.example.com/notes?token=abc')).toBe('preview.example.com/notes')
  })

  test('hides a path segment that looks like a token', () => {
    expect(shortUrl('https://staging.example.com/authlink/eyJhbGciOiJIUzI1NiJ9.x1y2')).toBe('staging.example.com/authlink/…')
  })

  test('keeps ordinary words in the path', () => {
    expect(shortUrl('https://staging.example.com/app/settings/calendar')).toBe('staging.example.com/app/settings/calendar')
  })
})
