import {describe, expect, test} from 'bun:test'
import {FONT_FILES} from '../video/overlays'
import {fontCopies, inputShimScript} from './worktree'

describe('inputShimScript', () => {
  test('keeps the driver beside the shim as cdp.real.mjs and puts the shim at the driver path', () => {
    expect(inputShimScript('.claude/skills/drive-desktop/cdp.mjs', '/Users/admin/qa-video')).toBe(
      "cd ~/wt/'.claude/skills/drive-desktop' && [[ -f 'cdp.mjs' && ! -f cdp.real.mjs ]] && mv 'cdp.mjs' cdp.real.mjs && cp '/Users/admin/qa-video'/cdp-shim.mjs 'cdp.mjs'"
    )
  })

  test('works for a driver at the worktree root', () => {
    expect(inputShimScript('driver.mjs', '/t')).toBe("cd ~/wt/'.' && [[ -f 'driver.mjs' && ! -f cdp.real.mjs ]] && mv 'driver.mjs' cdp.real.mjs && cp '/t'/cdp-shim.mjs 'driver.mjs'")
  })
})

describe('fontCopies', () => {
  test('maps each profile font to the file name the video editor reads', () => {
    const copies = fontCopies('/wt', '/fonts', {ui: 'f/Ui.woff2', uiBold: 'f/UiBold.woff2', mono: 'f/Mono.woff2'})
    expect(copies).toEqual([
      {from: '/wt/f/Ui.woff2', to: `/fonts/${FONT_FILES.ui}`},
      {from: '/wt/f/UiBold.woff2', to: `/fonts/${FONT_FILES.uiBold}`},
      {from: '/wt/f/Mono.woff2', to: `/fonts/${FONT_FILES.mono}`}
    ])
  })
})
