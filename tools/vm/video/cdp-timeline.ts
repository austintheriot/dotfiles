import {displayedText, ELEMENT_NAME_SOURCE, keyLabel, keyParts, screenPoint, type Point, type TimelineEvent} from './core'
import type {OsAction} from './os-input'
import {shortUrl, type Span} from './timeline'

export type CdpCall = {prefix: string[]; command: string; rest: string[]}
export type CdpProbe = {
  screenX: number
  screenY: number
  outerWidth: number
  outerHeight: number
  innerWidth: number
  innerHeight: number
  name: string
  fieldType: string
  autocomplete: string
  visible: boolean
  found: boolean
  rect?: Rect
}
type Rect = {x: number; y: number; width: number; height: number}

const COMMANDS = new Set(['targets', 'eval', 'shot', 'click', 'clicktext', 'key', 'type', 'setfile', 'tap', 'open'])
const MODIFIERS: Record<string, string> = {
  cmd: 'Meta',
  command: 'Meta',
  meta: 'Meta',
  shift: 'Shift',
  alt: 'Alt',
  opt: 'Alt',
  option: 'Alt',
  ctrl: 'Control',
  control: 'Control'
}

export function parseCdpCall(argv: string[]): CdpCall | undefined {
  const index = argv.findIndex(arg => COMMANDS.has(arg))
  if (index === -1) return undefined
  return {prefix: argv.slice(0, index), command: argv[index] ?? '', rest: argv.slice(index + 1)}
}

const TEXT_TARGET = (text: string) => `(() => {
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT);
      let node;
      while ((node = walker.nextNode())) {
        if (node.children.length === 0 && node.textContent.trim() === ${JSON.stringify(text)}) {
          return node.closest('button, a, [role=button], [role=menuitem]') ?? node;
        }
      }
      for (const input of document.querySelectorAll('input[type=button], input[type=submit], input[type=reset]')) {
        if (input.value.trim() === ${JSON.stringify(text)}) return input;
      }
      return null;
    })()`

function probeTarget(call: CdpCall): string | undefined {
  switch (call.command) {
    case 'click':
      return `document.querySelectorAll(${JSON.stringify(call.rest[0] ?? '')})[${Number(call.rest[1] ?? 0) || 0}]`
    case 'clicktext':
      return TEXT_TARGET(call.rest[0] ?? '')
    case 'type':
      return 'document.activeElement'
    case 'tap':
      return 'null'
    default:
      return undefined
  }
}

export function probeExpression(call: CdpCall): string | undefined {
  const target = probeTarget(call)
  if (target === undefined) return undefined
  return `(() => {
    const element = ${target};
    if (element && element !== document.activeElement) element.scrollIntoView({block: 'center'});
    const box = element ? element.getBoundingClientRect() : null;
    return {
      screenX: window.screenX, screenY: window.screenY,
      outerWidth: window.outerWidth, outerHeight: window.outerHeight,
      innerWidth: window.innerWidth, innerHeight: window.innerHeight,
      name: element ? (${ELEMENT_NAME_SOURCE})(element) : '',
      fieldType: element ? String(element.type || element.tagName).toLowerCase() : '',
      autocomplete: element ? String(element.autocomplete || '') : '',
      visible: document.visibilityState === 'visible',
      found: Boolean(element),
      ...(box ? {rect: {x: box.x, y: box.y, width: box.width, height: box.height}} : {})
    };
  })()`
}

export function openExpression(url: string): string | undefined {
  try {
    const parsed = new URL(url)
    return ['http:', 'https:'].includes(parsed.protocol) ? `location.href = ${JSON.stringify(parsed.href)}; undefined` : undefined
  } catch {
    return undefined
  }
}

export function cdpKey(key: string): string {
  return key
    .split(/\+(?!$)/)
    .map((part, index, parts) => (index < parts.length - 1 ? (MODIFIERS[part.toLowerCase()] ?? part) : part))
    .join('+')
}

export function parseCdpOutput(stdout: string): unknown {
  try {
    return JSON.parse(stdout)
  } catch {
    return undefined
  }
}

const record = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : undefined

export function parseCdpProbe(value: unknown): CdpProbe | undefined {
  const fields = record(value)
  if (!fields) return undefined
  const numbers = ['screenX', 'screenY', 'outerWidth', 'outerHeight', 'innerWidth', 'innerHeight'] as const
  if (!numbers.every(key => typeof fields[key] === 'number')) return undefined
  const text = (key: string) => (typeof fields[key] === 'string' ? fields[key] : '')
  return {
    screenX: Number(fields['screenX']),
    screenY: Number(fields['screenY']),
    outerWidth: Number(fields['outerWidth']),
    outerHeight: Number(fields['outerHeight']),
    innerWidth: Number(fields['innerWidth']),
    innerHeight: Number(fields['innerHeight']),
    name: text('name'),
    fieldType: text('fieldType'),
    autocomplete: text('autocomplete'),
    visible: fields['visible'] === true,
    found: fields['found'] === true,
    ...(parseRect(fields['rect']) ?? {})
  }
}

function parseRect(value: unknown): {rect: Rect} | undefined {
  const box = record(value)
  if (!box || !['x', 'y', 'width', 'height'].every(key => typeof box[key] === 'number')) return undefined
  return {rect: {x: Number(box['x']), y: Number(box['y']), width: Number(box['width']), height: Number(box['height'])}}
}

function viewportTarget(call: CdpCall, probe: CdpProbe | undefined): Point | undefined {
  if (call.command === 'tap') {
    const [x, y] = [Number(call.rest[0]), Number(call.rest[1])]
    return Number.isFinite(x) && Number.isFinite(y) ? {x, y} : undefined
  }
  const rect = probe?.found ? probe.rect : undefined
  return rect && {x: rect.x + rect.width / 2, y: rect.y + rect.height / 2}
}

export function cdpOsAction(call: CdpCall, probe: CdpProbe | undefined): OsAction | undefined {
  if (call.command === 'key') return {kind: 'key', key: cdpKey(call.rest[0] ?? '')}
  if (!probe?.visible) return undefined
  if (call.command === 'type') return {kind: 'type', text: call.rest[0] ?? '', submit: false}
  if (!['click', 'clicktext', 'tap'].includes(call.command)) return undefined
  const viewport = viewportTarget(call, probe)
  if (!viewport) return undefined
  return {kind: 'click', point: screenPoint({...probe, ...viewport, width: 0, height: 0}), button: 'left', double: false, modifiers: []}
}

export function cdpOutput(call: CdpCall, probe: CdpProbe | undefined): Record<string, unknown> {
  const [first = ''] = call.rest
  switch (call.command) {
    case 'tap':
      return {tapped: viewportTarget(call, probe)}
    case 'key':
      return {key: first}
    case 'type':
      return {typed: first}
    default: {
      const rect = probe?.found ? probe.rect : undefined
      if (!rect) return {error: call.command === 'clicktext' ? `no element with text ${first}` : `no element for ${first}`}
      return {clicked: first, at: {x: rect.x + rect.width / 2, y: rect.y + rect.height / 2, w: rect.width, h: rect.height}}
    }
  }
}

function viewportPoint(output: Record<string, unknown>): {x: number; y: number} | undefined {
  const at = record(output['at']) ?? record(output['tapped'])
  return at && typeof at['x'] === 'number' && typeof at['y'] === 'number' ? {x: at['x'], y: at['y']} : undefined
}

function pointOf(probe: CdpProbe | undefined, output: Record<string, unknown>) {
  const viewport = viewportPoint(output)
  if (!probe || !viewport) return {}
  return {point: screenPoint({...probe, ...viewport, width: 0, height: 0})}
}

function clickText(call: CdpCall, probe: CdpProbe | undefined): string {
  if (call.command === 'clicktext') return call.rest[0] ?? ''
  if (call.command === 'tap') return ''
  return probe?.name ?? ''
}

export function cdpEvent(call: CdpCall, probe: CdpProbe | undefined, output: unknown, span: Span): TimelineEvent | undefined {
  const result = record(output) ?? {}
  if ('error' in result) return undefined
  switch (call.command) {
    case 'click':
    case 'clicktext':
    case 'tap':
      return {kind: 'click', label: 'Click', text: clickText(call, probe), ...pointOf(probe, result), ...span}
    case 'key': {
      const key = cdpKey(call.rest[0] ?? '')
      return {kind: 'key', label: 'Pressed', text: keyLabel(key), keys: keyParts(key), ...span}
    }
    case 'type': {
      const field = probe ?? {fieldType: 'password', autocomplete: ''}
      return {kind: 'type', label: 'Typed', text: displayedText(call.rest[0] ?? '', field), ...span}
    }
    case 'setfile':
      return {kind: 'note', label: 'Uploaded', text: call.rest.slice(1).map(path => path.split('/').at(-1)).join(', '), ...span}
    case 'open':
      return {kind: 'navigate', label: 'Opened', text: shortUrl(call.rest[0] ?? ''), ...span}
    default:
      return undefined
  }
}
