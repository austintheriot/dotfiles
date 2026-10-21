import {describe, expect, test} from 'bun:test'
import {cdpEvent, cdpKey, cdpOsAction, cdpOutput, openExpression, parseCdpCall, parseCdpOutput, parseCdpProbe, probeExpression, type CdpProbe} from './cdp-timeline'
import {ELEMENT_NAME_SOURCE} from './core'

const span = {at: 10, endAt: 10.4}

function probe(overrides: Partial<CdpProbe> = {}): CdpProbe {
  return {
    screenX: 100,
    screenY: 40,
    outerWidth: 1440,
    outerHeight: 900,
    innerWidth: 1440,
    innerHeight: 900,
    name: 'Settings',
    fieldType: 'button',
    autocomplete: '',
    visible: true,
    found: true,
    rect: {x: 20, y: 50, width: 20, height: 20},
    ...overrides
  }
}

function unmeasured(overrides: Partial<CdpProbe> = {}): CdpProbe {
  const {rect, ...bare} = probe(overrides)
  return bare
}

describe('parseCdpCall', () => {
  test('splits the port and options from the command', () => {
    expect(parseCdpCall(['9224', '--url', 'notification', 'clicktext', 'Open note'])).toEqual({
      prefix: ['9224', '--url', 'notification'],
      command: 'clicktext',
      rest: ['Open note']
    })
  })

  test('keeps options that come before the port', () => {
    expect(parseCdpCall(['--host', '10.0.0.2', '9224', 'key', 'Enter'])?.prefix).toEqual(['--host', '10.0.0.2', '9224'])
  })

  test('stops at the first command word, so a typed "click" is text', () => {
    expect(parseCdpCall(['9224', 'type', 'click here'])).toEqual({prefix: ['9224'], command: 'type', rest: ['click here']})
  })

  test('returns undefined when no command is given', () => {
    expect(parseCdpCall(['9224'])).toBeUndefined()
  })
})

describe('probeExpression', () => {
  test('reads the clicked element and the window for a selector click', () => {
    const expression = probeExpression({prefix: ['9224'], command: 'click', rest: ['button[aria-label="Settings"]', '2']})
    expect(expression).toContain('querySelectorAll("button[aria-label=\\"Settings\\"]")[2]')
    expect(expression).toContain('screenX')
  })

  test('reads the focused element for typing', () => {
    expect(probeExpression({prefix: ['9224'], command: 'type', rest: ['hello']})).toContain('document.activeElement')
  })

  test('scrolls the clicked element into view before measuring it', () => {
    expect(probeExpression({prefix: ['9224'], command: 'click', rest: ['#b']})).toContain("scrollIntoView({block: 'center'})")
  })

  test('finds a clicktext target by its exact text', () => {
    expect(probeExpression({prefix: ['9224'], command: 'clicktext', rest: ['Notiz öffnen']})).toContain('"Notiz öffnen"')
  })

  test('clicktext also matches a button input by its value', () => {
    const expression = probeExpression({prefix: ['9333'], command: 'clicktext', rest: ['Continue']}) ?? ''
    expect(expression).toContain("input[type=button], input[type=submit], input[type=reset]")
    expect(expression).toContain('input.value.trim() === "Continue"')
  })

  test('names the target with the shared element name function', () => {
    expect(probeExpression({prefix: ['9224'], command: 'click', rest: ['#ContinueButton']})).toContain(ELEMENT_NAME_SOURCE)
  })

  test('reads only the window for a tap', () => {
    expect(probeExpression({prefix: ['9224'], command: 'tap', rest: ['10', '20']})).toContain('const element = null')
  })

  test('reads nothing for commands that are not input', () => {
    for (const command of ['eval', 'shot', 'targets', 'setfile']) {
      expect(probeExpression({prefix: ['9224'], command, rest: []})).toBeUndefined()
    }
  })
})

describe('cdpKey', () => {
  test('maps the driver modifier names to the editor key names', () => {
    expect(cdpKey('cmd+shift+o')).toBe('Meta+Shift+o')
    expect(cdpKey('ctrl+alt+k')).toBe('Control+Alt+k')
    expect(cdpKey('opt+ArrowDown')).toBe('Alt+ArrowDown')
  })

  test('leaves a plain key alone', () => {
    expect(cdpKey('Enter')).toBe('Enter')
    expect(cdpKey('+')).toBe('+')
  })
})

describe('parseCdpOutput', () => {
  test('reads the pretty-printed JSON the driver prints', () => {
    expect(parseCdpOutput('{\n "clicked": "x",\n "at": {"x": 1, "y": 2}\n}\n')).toEqual({clicked: 'x', at: {x: 1, y: 2}})
  })

  test('returns undefined for output that is not JSON', () => {
    expect(parseCdpOutput('no shell answering')).toBeUndefined()
  })
})

describe('cdpEvent', () => {
  test('places a selector click ring at the element centre on the screen', () => {
    const call = {prefix: ['9224'], command: 'click', rest: ['button[aria-label="Settings"]']}
    const output = {clicked: 'button[aria-label="Settings"]', at: {x: 30, y: 60, w: 20, h: 20}}
    expect(cdpEvent(call, probe(), output, span)).toEqual({kind: 'click', label: 'Click', text: 'Settings', point: {x: 130, y: 100}, ...span})
  })

  test('adds the title bar height and side border to the window origin', () => {
    const call = {prefix: ['9224'], command: 'tap', rest: ['30', '60']}
    const framed = probe({outerWidth: 1442, innerWidth: 1440, outerHeight: 928, innerHeight: 900})
    expect(cdpEvent(call, framed, {tapped: {x: 30, y: 60}}, span)?.point).toEqual({x: 131, y: 128})
  })

  test('names a clicktext click by its text', () => {
    const call = {prefix: ['9224', '--url', 'notification'], command: 'clicktext', rest: ['Notiz öffnen']}
    const event = cdpEvent(call, probe({name: ''}), {clicked: 'Notiz öffnen', at: {x: 5, y: 5, w: 10, h: 10}}, span)
    expect(event?.text).toBe('Notiz öffnen')
    expect(event?.point).toEqual({x: 105, y: 45})
  })

  test('logs a tap with no element name', () => {
    const event = cdpEvent({prefix: ['9224'], command: 'tap', rest: ['1076', '501']}, probe(), {tapped: {x: 1076, y: 501}}, span)
    expect(event).toEqual({kind: 'click', label: 'Click', text: '', point: {x: 1176, y: 541}, ...span})
  })

  test('logs a click with no ring when the window could not be read', () => {
    const event = cdpEvent({prefix: ['9224'], command: 'clicktext', rest: ['Trigger']}, undefined, {clicked: 'Trigger', at: {x: 1, y: 1}}, span)
    expect(event).toEqual({kind: 'click', label: 'Click', text: 'Trigger', ...span})
  })

  test('shows a key press as keycaps', () => {
    expect(cdpEvent({prefix: ['9224'], command: 'key', rest: ['cmd+shift+o']}, undefined, {key: 'cmd+shift+o'}, span)).toEqual({
      kind: 'key',
      label: 'Pressed',
      text: '⌘ ⇧ O',
      keys: ['⌘', '⇧', 'O'],
      ...span
    })
  })

  test('shows typed text, and masks it in a password field', () => {
    const call = {prefix: ['9224'], command: 'type', rest: ['hunter22']}
    expect(cdpEvent(call, probe({fieldType: 'text'}), {typed: 'hunter22'}, span)?.text).toBe('hunter22')
    expect(cdpEvent(call, probe({fieldType: 'password'}), {typed: 'hunter22'}, span)?.text).toBe('••••••••')
  })

  test('masks typed text when the focused field could not be read', () => {
    expect(cdpEvent({prefix: ['9224'], command: 'type', rest: ['secret']}, undefined, {typed: 'secret'}, span)?.text).toBe('••••••')
  })

  test('names uploaded files by their base name', () => {
    const call = {prefix: ['9224'], command: 'setfile', rest: ['input[type=file]', '/tmp/a/one.pdf', '/tmp/two.png']}
    expect(cdpEvent(call, undefined, {}, span)).toEqual({kind: 'note', label: 'Uploaded', text: 'one.pdf, two.png', ...span})
  })

  test('logs nothing for a failed input or a command that is not input', () => {
    expect(cdpEvent({prefix: ['9224'], command: 'click', rest: ['#gone']}, probe(), {error: 'no element for #gone'}, span)).toBeUndefined()
    expect(cdpEvent({prefix: ['9224'], command: 'eval', rest: ['1']}, undefined, 1, span)).toBeUndefined()
    expect(cdpEvent({prefix: ['9224'], command: 'shot', rest: ['/tmp/a.png']}, undefined, {saved: '/tmp/a.png'}, span)).toBeUndefined()
  })
})

describe('parseCdpProbe', () => {
  test('keeps the element rectangle and the window state', () => {
    const raw = {...probe(), rect: {x: 1, y: 2, width: 3, height: 4}, visible: false, found: true}
    expect(parseCdpProbe(raw)).toEqual({...probe(), rect: {x: 1, y: 2, width: 3, height: 4}, visible: false})
  })

  test('rejects a result without window metrics', () => {
    expect(parseCdpProbe({name: 'x'})).toBeUndefined()
  })
})

describe('cdpOsAction', () => {
  test('clicks the element centre on the screen', () => {
    const call = {prefix: ['9224'], command: 'click', rest: ['#b']}
    expect(cdpOsAction(call, probe())).toEqual({kind: 'click', point: {x: 130, y: 100}, button: 'left', double: false, modifiers: []})
  })

  test('clicks the raw viewport point of a tap', () => {
    const call = {prefix: ['9224'], command: 'tap', rest: ['1076', '501']}
    expect(cdpOsAction(call, unmeasured())).toMatchObject({kind: 'click', point: {x: 1176, y: 541}})
  })

  test('presses a key without measuring anything', () => {
    expect(cdpOsAction({prefix: ['9224'], command: 'key', rest: ['cmd+shift+o']}, undefined)).toEqual({kind: 'key', key: 'Meta+Shift+o'})
  })

  test('types into the focused field', () => {
    expect(cdpOsAction({prefix: ['9224'], command: 'type', rest: ['hi']}, probe())).toEqual({kind: 'type', text: 'hi', submit: false})
  })

  test('gives no OS action for a hidden window, a missing element, or an unmeasured click', () => {
    const call = {prefix: ['9224'], command: 'click', rest: ['#b']}
    expect(cdpOsAction(call, probe({visible: false}))).toBeUndefined()
    expect(cdpOsAction(call, unmeasured({found: false}))).toBeUndefined()
    expect(cdpOsAction(call, undefined)).toBeUndefined()
    expect(cdpOsAction({prefix: ['9224'], command: 'setfile', rest: ['#f', '/a']}, probe())).toBeUndefined()
  })
})

describe('cdpOutput', () => {
  test('prints what the original driver prints', () => {
    expect(cdpOutput({prefix: ['9224'], command: 'click', rest: ['#b']}, probe())).toEqual({clicked: '#b', at: {x: 30, y: 60, w: 20, h: 20}})
    expect(cdpOutput({prefix: ['9224'], command: 'tap', rest: ['5', '6']}, probe())).toEqual({tapped: {x: 5, y: 6}})
    expect(cdpOutput({prefix: ['9224'], command: 'key', rest: ['Enter']}, undefined)).toEqual({key: 'Enter'})
    expect(cdpOutput({prefix: ['9224'], command: 'type', rest: ['hi']}, probe())).toEqual({typed: 'hi'})
  })

  test('reports a missing element the way the original driver does', () => {
    expect(cdpOutput({prefix: ['9224'], command: 'click', rest: ['#gone']}, probe({found: false}))).toEqual({error: 'no element for #gone'})
    expect(cdpOutput({prefix: ['9224'], command: 'clicktext', rest: ['Nope']}, probe({found: false}))).toEqual({error: 'no element with text Nope'})
  })
})

describe('clicktext logging', () => {
  const call = {prefix: ['9224'], command: 'clicktext', rest: ['Import materials']}

  test('every clicktext the OS path sends has a click event with its text and a ring point', () => {
    const measured = probe({name: 'Import'})
    expect(cdpOsAction(call, measured)?.kind).toBe('click')
    const event = cdpEvent(call, measured, cdpOutput(call, measured), span)
    expect(event).toMatchObject({kind: 'click', label: 'Click', text: 'Import materials'})
    expect(event?.point).toBeDefined()
  })

  test('a clicktext the CDP fallback sends is logged from the driver output even when the probe failed', () => {
    const output = {clicked: 'Import materials', at: {x: 30, y: 60, w: 20, h: 20}}
    expect(cdpOsAction(call, undefined)).toBeUndefined()
    expect(cdpEvent(call, undefined, output, span)).toMatchObject({kind: 'click', text: 'Import materials'})
  })

  test('a clicktext in a hidden window falls back to CDP and is still logged', () => {
    const hidden = probe({visible: false})
    expect(cdpOsAction(call, hidden)).toBeUndefined()
    expect(cdpEvent(call, hidden, cdpOutput(call, hidden), span)).toMatchObject({kind: 'click', text: 'Import materials'})
  })

  test('a clicktext that finds no element sends nothing and logs nothing', () => {
    const missing = probe({found: false})
    expect(cdpOsAction(call, missing)).toBeUndefined()
    expect(cdpEvent(call, missing, cdpOutput(call, missing), span)).toBeUndefined()
  })
})

describe('open', () => {
  test('parses open as a command', () => {
    expect(parseCdpCall(['9333', 'open', 'https://staging.example.com/login'])).toEqual({
      prefix: ['9333'],
      command: 'open',
      rest: ['https://staging.example.com/login']
    })
  })

  test('navigates only to http and https URLs', () => {
    expect(openExpression('https://staging.example.com/login')).toBe('location.href = "https://staging.example.com/login"; undefined')
    expect(openExpression('javascript:alert(1)')).toBeUndefined()
    expect(openExpression('not a url')).toBeUndefined()
  })

  test('logs an Opened chip with no query and no token', () => {
    const call = {prefix: ['9333'], command: 'open', rest: ['https://staging.example.com/authlink/eyJhbGciOiJIUzI1NiJ9.x1y2?next=%2Fapp']}
    expect(cdpEvent(call, undefined, {}, span)).toEqual({kind: 'navigate', label: 'Opened', text: 'staging.example.com/authlink/…', ...span})
  })
})
