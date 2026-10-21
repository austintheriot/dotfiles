import type {InlineComment} from './payload'

export type LineRange = {start: number; end: number}
export type HunkRanges = {right: LineRange[]; left: LineRange[]}
export type PullFile = {filename: string; patch: string | undefined}

const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/

const rangeOf = (start: string, count: string | undefined): LineRange[] => {
  const lines = count === undefined ? 1 : Number(count)
  return lines === 0 ? [] : [{start: Number(start), end: Number(start) + lines - 1}]
}

// The new-side count of a hunk header covers its context and added lines, and GitHub accepts a RIGHT comment on exactly those lines.
export function parsePatch(patch: string): HunkRanges {
  const headers = patch.split('\n').flatMap(line => {
    const match = HUNK_HEADER.exec(line)
    return match === null ? [] : [match]
  })
  return {
    right: headers.flatMap(match => rangeOf(match[3] ?? '0', match[4])),
    left: headers.flatMap(match => rangeOf(match[1] ?? '0', match[2]))
  }
}

const where = (comment: InlineComment) =>
  comment.startLine === undefined || comment.startLine === comment.line ? `line ${comment.line}` : `lines ${comment.startLine}-${comment.line}`

const insideOneRange = (ranges: readonly LineRange[], first: number, last: number) => ranges.some(range => range.start <= first && last <= range.end)

function problemFor(comment: InlineComment, files: readonly PullFile[]): string | undefined {
  const subject = `the inline comment on ${comment.path} ${where(comment)} (${comment.side}) from ${comment.source}`
  const file = files.find(candidate => candidate.filename === comment.path)
  if (file === undefined) return `${subject} is on a file that the pull request does not change`
  if (file.patch === undefined) return `${subject} is on a file that GitHub sends with no patch, so draft-review cannot check the line`
  const hunks = parsePatch(file.patch)
  const ranges = comment.side === 'RIGHT' ? hunks.right : hunks.left
  if (insideOneRange(ranges, comment.startLine ?? comment.line, comment.line)) return undefined
  return `${subject} is outside every hunk of the pull request diff on the ${comment.side} side`
}

export const anchorProblems = (comments: readonly InlineComment[], files: readonly PullFile[]): string[] =>
  comments.flatMap(comment => {
    const problem = problemFor(comment, files)
    return problem === undefined ? [] : [problem]
  })
