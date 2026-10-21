import {displayedText, ELEMENT_NAME_SOURCE, keyLabel, keyParts, screenPoint, type ElementBox, type TimelineEvent} from './core'
import type {OsAction} from './os-input'

export type Probe = ElementBox & {fieldType: string; autocomplete: string; name: string; inFrame: boolean; visible: boolean}
export type Span = {at: number; endAt: number}
type Args = Record<string, unknown>

const FORM_TEXT_MAX = 96

export const PROBE_FUNCTION = `(element) => {
  element.scrollIntoView({block: 'nearest', inline: 'nearest'});
  const rect = element.getBoundingClientRect();
  return {
    x: rect.x, y: rect.y, width: rect.width, height: rect.height,
    screenX: window.screenX, screenY: window.screenY,
    outerWidth: window.outerWidth, outerHeight: window.outerHeight,
    innerWidth: window.innerWidth, innerHeight: window.innerHeight,
    fieldType: String(element.type || element.tagName).toLowerCase(),
    autocomplete: String(element.autocomplete || ''),
    name: (${ELEMENT_NAME_SOURCE})(element),
    inFrame: window !== window.top,
    visible: document.visibilityState === 'visible'
  };
}`

const text = (args: Args, key: string): string | undefined => (typeof args[key] === 'string' ? args[key] : undefined)
const strings = (args: Args, key: string): string[] => {
  const value = args[key]
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []
}

type FormField = {target: string; name: string; type: string; value: string}
function formFields(args: Args): FormField[] {
  const fields = args['fields']
  if (!Array.isArray(fields)) return []
  return fields.flatMap(field => {
    if (typeof field !== 'object' || field === null) return []
    const record = field as Args
    const [target, name, type, value] = ['target', 'name', 'type', 'value'].map(key => text(record, key))
    if (target === undefined || name === undefined || type === undefined || value === undefined) return []
    return [{target, name, type, value}]
  })
}

export function probeTargets(tool: string, args: Args): string[] {
  if (tool === 'browser_fill_form') return formFields(args).map(field => field.target)
  if (['browser_click', 'browser_type', 'browser_select_option', 'browser_hover'].includes(tool)) {
    const target = text(args, 'target')
    return target === undefined ? [] : [target]
  }
  return []
}

export function parseProbe(resultText: string): Probe | undefined {
  const match = /### Result\n([\s\S]*?)(?:\n### |$)/.exec(resultText)
  if (!match?.[1]) return undefined
  try {
    const value: unknown = JSON.parse(match[1])
    return typeof value === 'object' && value !== null ? (value as Probe) : undefined
  } catch {
    return undefined
  }
}

const isPasswordProbe = (probe: Probe | undefined) =>
  probe === undefined || probe.fieldType === 'password' || probe.autocomplete.includes('password')

function maskedText(value: string, probe: Probe | undefined, fieldName = ''): string {
  const fieldType = isPasswordProbe(probe) || /password|passcode|\bpin\b/i.test(fieldName) ? 'password' : 'text'
  return displayedText(value, {fieldType})
}

const shortened = (value: string, max: number) => (value.length > max ? `${value.slice(0, max - 1)}…` : value)

const pointOf = (probe: Probe | undefined) => (probe && !probe.inFrame ? {point: screenPoint(probe)} : {})

// A magic link can carry its token in the path, and a chip is posted.
const looksLikeToken = (segment: string) => segment.length >= 20 && /^[A-Za-z0-9._~-]+$/.test(segment) && /\d/.test(segment)

export function shortUrl(url: string): string {
  try {
    const parsed = new URL(url)
    if (!['http:', 'https:'].includes(parsed.protocol)) return `${parsed.protocol.replace(':', '')} URL`
    const path = parsed.pathname
      .split('/')
      .map(segment => (looksLikeToken(segment) ? '…' : segment))
      .join('/')
    return `${parsed.host}${path === '/' ? '' : path}`
  } catch {
    return 'page'
  }
}

function clickLabel(args: Args): string {
  if (args['doubleClick'] === true) return 'Double-click'
  if (args['button'] === 'right') return 'Right-click'
  return 'Click'
}

function formValue(field: FormField, probe: Probe | undefined): string {
  if (field.type === 'checkbox') return field.value === 'true' ? 'on' : 'off'
  if (field.type === 'textbox') return maskedText(field.value, probe, field.name)
  return field.value
}

export function eventFor(tool: string, args: Args, probes: Array<Probe | undefined>, span: Span): TimelineEvent | undefined {
  const [first] = probes
  switch (tool) {
    case 'browser_click': {
      const modifiers = strings(args, 'modifiers')
      const prefix = modifiers.length > 0 ? `${keyLabel(modifiers.join('+'))} ` : ''
      return {kind: 'click', label: clickLabel(args), text: `${prefix}${first?.name ?? ''}`.trim(), ...pointOf(first), ...span}
    }
    case 'browser_type': {
      const typed = maskedText(text(args, 'text') ?? '', first)
      return {kind: 'type', label: 'Typed', text: args['submit'] === true ? `${typed} ↩` : typed, ...pointOf(first), ...span}
    }
    case 'browser_select_option':
      return {kind: 'type', label: 'Selected', text: strings(args, 'values').join(', '), ...pointOf(first), ...span}
    case 'browser_press_key': {
      const key = text(args, 'key') ?? ''
      return {kind: 'key', label: 'Pressed', text: keyLabel(key), keys: keyParts(key), ...span}
    }
    case 'browser_fill_form': {
      const summary = formFields(args)
        .map((field, index) => `${field.name}: ${formValue(field, probes[index])}`)
        .join(' · ')
      return {kind: 'type', label: 'Filled', text: shortened(summary, FORM_TEXT_MAX), ...span}
    }
    case 'browser_navigate':
      return {kind: 'navigate', label: 'Opened', text: shortUrl(text(args, 'url') ?? ''), ...span}
    case 'browser_navigate_back':
      return {kind: 'navigate', label: 'Back', ...span}
    case 'browser_file_upload':
      return {kind: 'note', label: 'Uploaded', text: strings(args, 'paths').map(path => path.split('/').at(-1)).join(', '), ...span}
    case 'browser_handle_dialog':
      return {kind: 'click', label: 'Dialog', text: args['accept'] === true ? 'Accepted' : 'Dismissed', ...span}
    default:
      return undefined
  }
}

const reachable = (probe: Probe | undefined) => (probe && !probe.inFrame && probe.visible ? screenPoint(probe) : undefined)

export function osActionFor(tool: string, args: Args, probes: Array<Probe | undefined>): OsAction | undefined {
  if (tool === 'browser_press_key') {
    const key = text(args, 'key')
    return key ? {kind: 'key', key} : undefined
  }
  const point = reachable(probes[0])
  if (!point) return undefined
  switch (tool) {
    case 'browser_click': {
      const button = args['button'] ?? 'left'
      if (button !== 'left' && button !== 'right') return undefined
      return {kind: 'click', point, button, double: args['doubleClick'] === true, modifiers: strings(args, 'modifiers')}
    }
    case 'browser_type':
      return {kind: 'type', text: text(args, 'text') ?? '', submit: args['submit'] === true, field: point}
    case 'browser_hover':
      return {kind: 'move', point}
    default:
      return undefined
  }
}
