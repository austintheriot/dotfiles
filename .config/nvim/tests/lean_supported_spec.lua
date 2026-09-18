local t = require '_harness'
local m = require 'dotfiles.lean_supported'

t.eq(m.is_supported '0.11.5', true, 'the exact floor is supported')
t.eq(m.is_supported '0.12.4', true, 'a version above the floor is supported')
t.eq(m.is_supported '0.11.4', false, 'the version just below the floor is not')
t.eq(m.is_supported '0.10.0', false, 'the floor this config enforces elsewhere is not enough for lean.nvim')

-- A prerelease of the floor sorts BELOW the floor under semver, and that is
-- the answer we want: 0.11.5-dev does not yet contain what 0.11.5 ships.
t.eq(m.is_supported '0.11.5-dev', false, 'a prerelease of the floor is not the floor')
t.eq(m.is_supported '0.12.0-dev+g1234', true, 'a nightly above the floor is supported')

-- vim.version.parse rejects these rather than returning something ordered,
-- so the guard has to answer rather than raise. Refusing to enable is the
-- safe answer: lean.nvim only WARNS on an old Neovim and then fails later
-- with unrelated errors, which is the outcome this module exists to avoid.
t.eq(m.is_supported 'not a version', false, 'an unparseable version is not supported')
t.eq(m.is_supported '', false, 'an empty version is not supported')
t.eq(m.is_supported(nil), false, 'a nil version is not supported')

t.eq(m.floor, '0.11.5', "the floor is lean.nvim's own MIN_SUPPORTED_NVIM")

t.finish 'lean_supported'
