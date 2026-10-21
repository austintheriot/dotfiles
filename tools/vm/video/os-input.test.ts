import {describe, expect, test} from 'bun:test'
import {cliclickKey, cliclickPlan, keystrokeScript, systemEventsKeystroke} from './os-input'

describe('cliclickKey', () => {
  test('maps named keys to cliclick key names', () => {
    expect(cliclickKey('Enter')).toEqual(['kp:return'])
    expect(cliclickKey('Escape')).toEqual(['kp:esc'])
    expect(cliclickKey('Backspace')).toEqual(['kp:delete'])
    expect(cliclickKey('Delete')).toEqual(['kp:fwd-delete'])
    expect(cliclickKey('ArrowDown')).toEqual(['kp:arrow-down'])
    expect(cliclickKey('PageUp')).toEqual(['kp:page-up'])
    expect(cliclickKey(' ')).toEqual(['kp:space'])
    expect(cliclickKey('F5')).toEqual(['kp:f5'])
  })

  test('types a single character', () => {
    expect(cliclickKey('a')).toEqual(['t:a'])
    expect(cliclickKey('+')).toEqual(['t:+'])
  })

  test('holds modifiers around the key', () => {
    expect(cliclickKey('Meta+Shift+o')).toEqual(['kd:cmd,shift', 't:o', 'ku:cmd,shift'])
    expect(cliclickKey('ControlOrMeta+Enter')).toEqual(['kd:cmd', 'kp:return', 'ku:cmd'])
    expect(cliclickKey('Control+Alt+ArrowLeft')).toEqual(['kd:ctrl,alt', 'kp:arrow-left', 'ku:ctrl,alt'])
  })

  test('refuses a key cliclick cannot send', () => {
    expect(cliclickKey('MediaTrackNext')).toBeUndefined()
    expect(cliclickKey('Meta+')).toBeUndefined()
  })
})

describe('cliclickPlan', () => {
  test('glides to the target, then clicks it', () => {
    expect(cliclickPlan({kind: 'click', point: {x: 100.4, y: 200.6}, button: 'left', double: false, modifiers: []})).toEqual({
      glide: ['-e', '40', 'm:100,201'],
      act: ['c:100,201']
    })
  })

  test('double-clicks and right-clicks', () => {
    expect(cliclickPlan({kind: 'click', point: {x: 1, y: 2}, button: 'left', double: true, modifiers: []})?.act).toEqual(['dc:1,2'])
    expect(cliclickPlan({kind: 'click', point: {x: 1, y: 2}, button: 'right', double: false, modifiers: []})?.act).toEqual(['rc:1,2'])
  })

  test('holds modifiers during a click', () => {
    expect(cliclickPlan({kind: 'click', point: {x: 1, y: 2}, button: 'left', double: false, modifiers: ['Meta', 'Shift']})?.act).toEqual([
      'kd:cmd,shift',
      'c:1,2',
      'ku:cmd,shift'
    ])
  })

  test('refuses a modifier cliclick cannot hold', () => {
    expect(cliclickPlan({kind: 'click', point: {x: 1, y: 2}, button: 'left', double: false, modifiers: ['Hyper']})).toBeUndefined()
  })

  test('only glides for a hover', () => {
    expect(cliclickPlan({kind: 'move', point: {x: 5, y: 6}})).toEqual({glide: ['-e', '40', 'm:5,6'], act: []})
  })

  test('types into the focused field, one line at a time', () => {
    expect(cliclickPlan({kind: 'type', text: 'one\ntwo', submit: false})).toEqual({glide: [], act: ['t:one', 'kp:return', 't:two']})
  })

  test('replaces the text of a field it clicks first, then submits', () => {
    expect(cliclickPlan({kind: 'type', text: 'hi', submit: true, field: {x: 10, y: 20}})).toEqual({
      glide: ['-e', '40', 'm:10,20'],
      act: ['c:10,20', 'kd:cmd', 't:a', 'ku:cmd', 't:hi', 'kp:return']
    })
  })

  test('refuses text cliclick cannot type, so it never types it wrong', () => {
    expect(cliclickPlan({kind: 'type', text: 'Notiz öffnen', submit: false})).toBeUndefined()
    expect(cliclickPlan({kind: 'key', key: 'é'})).toBeUndefined()
  })

  test('sends no empty type command', () => {
    expect(cliclickPlan({kind: 'type', text: '', submit: true})?.act).toEqual(['kp:return'])
  })

  test('presses a key', () => {
    expect(cliclickPlan({kind: 'key', key: 'Escape'})).toEqual({glide: [], act: ['kp:esc']})
    expect(cliclickPlan({kind: 'key', key: 'MediaTrackNext'})).toBeUndefined()
  })
})

describe('systemEventsKeystroke', () => {
  test('sends a modifier plus punctuation chord as a System Events keystroke', () => {
    expect(systemEventsKeystroke('Meta+-')).toEqual({character: '-', keyCode: 27, modifiers: ['command']})
    expect(systemEventsKeystroke('Meta+=')).toEqual({character: '=', keyCode: 24, modifiers: ['command']})
    expect(systemEventsKeystroke('Meta+Shift+=')).toEqual({character: '=', keyCode: 24, modifiers: ['command', 'shift']})
    expect(systemEventsKeystroke('ControlOrMeta+-')).toEqual({character: '-', keyCode: 27, modifiers: ['command']})
    expect(systemEventsKeystroke('Control+Alt+,')).toEqual({character: ',', keyCode: 43, modifiers: ['control', 'option']})
  })

  test('presses the physical key for punctuation, because keystroke of = with command does not reach Chrome', () => {
    expect(systemEventsKeystroke('Meta+=')?.keyCode).toBe(24)
    expect(systemEventsKeystroke('Meta+/')?.keyCode).toBe(44)
    expect(systemEventsKeystroke('Meta+[')?.keyCode).toBe(33)
  })

  test('reads a plus key after the separator', () => {
    expect(systemEventsKeystroke('Meta++')).toEqual({character: '+', keyCode: undefined, modifiers: ['command']})
  })

  test('sends a modifier plus digit chord the same way', () => {
    expect(systemEventsKeystroke('Meta+0')).toEqual({character: '0', keyCode: undefined, modifiers: ['command']})
  })

  test('leaves letters, named keys, bare punctuation, and shift-only chords to cliclick', () => {
    expect(systemEventsKeystroke('Meta+o')).toBeUndefined()
    expect(systemEventsKeystroke('Meta+Shift+o')).toBeUndefined()
    expect(systemEventsKeystroke('Meta+Enter')).toBeUndefined()
    expect(systemEventsKeystroke('-')).toBeUndefined()
    expect(systemEventsKeystroke('Shift+=')).toBeUndefined()
    expect(systemEventsKeystroke('0')).toBeUndefined()
  })

  test('refuses a modifier it cannot name and a character outside ASCII', () => {
    expect(systemEventsKeystroke('Hyper+-')).toBeUndefined()
    expect(systemEventsKeystroke('Meta+é')).toBeUndefined()
  })

  test('keeps such a chord out of the cliclick plan', () => {
    expect(cliclickKey('Meta+-')).toBeUndefined()
    expect(cliclickPlan({kind: 'key', key: 'Meta+='})).toBeUndefined()
    expect(cliclickKey('Meta+Shift+o')).toEqual(['kd:cmd,shift', 't:o', 'ku:cmd,shift'])
  })
})

describe('keystrokeScript', () => {
  test('presses the key code when the plan has one', () => {
    expect(keystrokeScript({character: '=', keyCode: 24, modifiers: ['command', 'shift']})).toBe(
      'tell application "System Events" to key code 24 using {command down, shift down}'
    )
  })

  test('types the character when the plan has no key code', () => {
    expect(keystrokeScript({character: '+', keyCode: undefined, modifiers: ['command']})).toBe(
      'tell application "System Events" to keystroke "+" using {command down}'
    )
  })

  test('escapes a quote and a backslash in the key', () => {
    expect(keystrokeScript({character: '"', keyCode: undefined, modifiers: ['command']})).toContain('keystroke "\\"" using')
    expect(keystrokeScript({character: '\\', keyCode: undefined, modifiers: ['command']})).toContain('keystroke "\\\\" using')
  })
})
