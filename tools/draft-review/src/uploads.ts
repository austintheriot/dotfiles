import {unique} from './lists'
import type {ReviewPayload} from './synthesize'

const escapeForRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

// A path matches only as a whole: `/a.png` must not match inside `/a.png.mp4` or `/x/a.png`, but must match before a sentence-ending period.
function pathMatcher(paths: readonly string[]): RegExp | undefined {
  const longestFirst = unique(paths.filter(path => path.length > 0)).sort((left, right) => right.length - left.length)
  if (longestFirst.length === 0) return undefined
  return new RegExp(`(?<![\\w./-])(?:${longestFirst.map(escapeForRegExp).join('|')})(?![\\w/-]|\\.\\w)`, 'g')
}

export function citedPaths(texts: readonly string[], candidates: readonly string[]): string[] {
  const matcher = pathMatcher(candidates)
  if (matcher === undefined) return []
  return unique([...texts.join('\n').matchAll(matcher)].map(match => match[0]))
}

export function substituteUploads(payload: ReviewPayload, urlByPath: ReadonlyMap<string, string>): ReviewPayload {
  const matcher = pathMatcher([...urlByPath.keys()])
  if (matcher === undefined) return payload
  const substitute = (text: string) => text.replace(matcher, path => urlByPath.get(path) ?? path)
  return {
    ...payload,
    body: substitute(payload.body),
    comments: payload.comments.map(comment => ({...comment, body: substitute(comment.body)}))
  }
}
