export type InlineComment = {path: string; line: number; side: 'LEFT' | 'RIGHT'; startLine?: number; body: string; source: string}
export type ReviewPayload = {body: string; comments: InlineComment[]; uploads: string[]; handback: string[]}

export const lineLabel = (comment: InlineComment): string =>
  comment.startLine === undefined || comment.startLine === comment.line ? `line ${comment.line}` : `lines ${comment.startLine}-${comment.line}`
