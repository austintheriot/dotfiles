import type {Point} from './core'

export type OsAction =
  | {kind: 'click'; point: Point; button: 'left' | 'right'; double: boolean; modifiers: string[]}
  | {kind: 'move'; point: Point}
  | {kind: 'key'; key: string}
  | {kind: 'type'; text: string; submit: boolean; field?: Point}

// cliclick easing factor, not a duration: 40 moves 1300 points in about 0.6 s in the QA VM.
const GLIDE_EASING = 40

const MODIFIERS: Record<string, string> = {
  Meta: 'cmd',
  ControlOrMeta: 'cmd',
  Control: 'ctrl',
  Alt: 'alt',
  Shift: 'shift'
}

const NAMED_KEYS: Record<string, string> = {
  Enter: 'return',
  Escape: 'esc',
  Tab: 'tab',
  Backspace: 'delete',
  Delete: 'fwd-delete',
  ArrowLeft: 'arrow-left',
  ArrowRight: 'arrow-right',
  ArrowUp: 'arrow-up',
  ArrowDown: 'arrow-down',
  PageUp: 'page-up',
  PageDown: 'page-down',
  Home: 'home',
  End: 'end',
  Space: 'space',
  ' ': 'space'
}

// cliclick types only characters on the US keyboard layout. It skips any other character and still exits 0.
const TYPEABLE = /^[\x20-\x7E\n]*$/

const at = (point: Point) => `${Math.round(point.x)},${Math.round(point.y)}`
const glideTo = (point: Point) => ['-e', String(GLIDE_EASING), `m:${at(point)}`]

function held(modifiers: string[], inner: string[]): string[] | undefined {
  const names = modifiers.map(modifier => MODIFIERS[modifier])
  if (names.some(name => name === undefined)) return undefined
  if (names.length === 0) return inner
  const list = names.join(',')
  return [`kd:${list}`, ...inner, `ku:${list}`]
}

function plainKey(key: string): string | undefined {
  const named = NAMED_KEYS[key]
  if (named) return `kp:${named}`
  if (/^F([1-9]|1[0-6])$/.test(key)) return `kp:${key.toLowerCase()}`
  return [...key].length === 1 && TYPEABLE.test(key) ? `t:${key}` : undefined
}

const SYSTEM_EVENTS_MODIFIERS: Record<string, string> = {
  Meta: 'command',
  ControlOrMeta: 'command',
  Control: 'control',
  Alt: 'option',
  Shift: 'shift'
}

export type SystemEventsKeystroke = {character: string; keyCode: number | undefined; modifiers: string[]}

// US ANSI virtual key codes. In the VM, keystroke "=" with command down did not zoom Chrome, but key code 24 did.
const PUNCTUATION_KEY_CODES: Record<string, number> = {
  '-': 27,
  '=': 24,
  '[': 33,
  ']': 30,
  ';': 41,
  "'": 39,
  ',': 43,
  '.': 47,
  '/': 44,
  '\\': 42,
  '`': 50
}

// Modifier plus punctuation or digit chords were observed or assumed to fail through cliclick, though modifier plus
// letter works through the same kd:/t: path. The cause is unknown, so those chords go through System Events.
export function systemEventsKeystroke(key: string): SystemEventsKeystroke | undefined {
  const parts = key.split(/\+(?!$)/)
  const character = parts.at(-1) ?? ''
  const modifierNames = parts.slice(0, -1)
  const modifiers = modifierNames.map(name => SYSTEM_EVENTS_MODIFIERS[name])
  const printable = [...character].length === 1 && TYPEABLE.test(character) && character !== '\n' && character !== ' '
  if (!printable || /^[A-Za-z]$/.test(character)) return undefined
  if (modifiers.some(modifier => modifier === undefined)) return undefined
  const named = modifiers.filter((modifier): modifier is string => modifier !== undefined)
  if (named.every(modifier => modifier === 'shift')) return undefined
  return {character, keyCode: PUNCTUATION_KEY_CODES[character], modifiers: [...new Set(named)]}
}

const appleScriptString = (text: string) => `"${text.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`

export function keystrokeScript(keystroke: SystemEventsKeystroke): string {
  const held = keystroke.modifiers.map(modifier => `${modifier} down`).join(', ')
  const press = keystroke.keyCode === undefined ? `keystroke ${appleScriptString(keystroke.character)}` : `key code ${keystroke.keyCode}`
  return `tell application "System Events" to ${press} using {${held}}`
}

export function cliclickKey(key: string): string[] | undefined {
  if (systemEventsKeystroke(key)) return undefined
  const parts = key.split(/\+(?!$)/)
  const last = parts.at(-1) ?? ''
  const press = last === '' ? undefined : plainKey(last)
  return press === undefined ? undefined : held(parts.slice(0, -1), [press])
}

function typing(text: string): string[] {
  return text.split('\n').flatMap((line, index) => [...(index > 0 ? ['kp:return'] : []), ...(line ? [`t:${line}`] : [])])
}

export type CliclickPlan = {glide: string[]; act: string[]}

export function cliclickPlan(action: OsAction): CliclickPlan | undefined {
  switch (action.kind) {
    case 'move':
      return {glide: glideTo(action.point), act: []}
    case 'click': {
      const command = action.button === 'right' ? 'rc' : action.double ? 'dc' : 'c'
      const click = held(action.modifiers, [`${command}:${at(action.point)}`])
      return click && {glide: glideTo(action.point), act: click}
    }
    case 'key': {
      const keys = cliclickKey(action.key)
      return keys && {glide: [], act: keys}
    }
    case 'type': {
      if (!TYPEABLE.test(action.text)) return undefined
      const replace = action.field ? [`c:${at(action.field)}`, 'kd:cmd', 't:a', 'ku:cmd'] : []
      return {
        glide: action.field ? glideTo(action.field) : [],
        act: [...replace, ...typing(action.text), ...(action.submit ? ['kp:return'] : [])]
      }
    }
  }
}
