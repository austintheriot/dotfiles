export type InlineComment = {path: string; line: number; side: 'LEFT' | 'RIGHT'; startLine?: number; body: string; source: string}
export type ReviewPayload = {body: string; comments: InlineComment[]; uploads: string[]; handback: string[]}
