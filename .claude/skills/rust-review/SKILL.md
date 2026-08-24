---
name: rust-review
description: Expert-level Rust review pass focused on idiomatic design, safety, and the language-specific footguns that `rustc` and `clippy` don't catch. Reviews the current branch diff against main by default, a specific file/PR with `/rust-review <path>` or `/rust-review <PR#>`, or a git range. Auto-routes hunks to specialist subagents (rust-async / rust-unsafe / rust-wasm / rust-ffi / rust-backend) based on what changed. Produces severity-labeled findings with file:line references. Does NOT post comments. Use when asked for a Rust review, soundness audit, type/API critique, or expert Rust opinion on changed code.
---

# Rust Review

You are doing an **expert-level Rust review**. The user wants design and idiom advice that goes beyond what `rustc` and `clippy` already report. Do NOT re-report compiler or lint errors.

The rule file `~/.claude/rules/rust.md` is your baseline checklist. The five specialist subagents (`rust-async`, `rust-backend`, `rust-unsafe`, `rust-wasm`, `rust-ffi` -- see `~/.claude/agents/`) own their respective domains. Cross-cutting principles live in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`.

## Scope resolution

- **No arg / `<PR#>` / `<range>`** -- delegate to the `pr-diff` agent (`Agent` tool, `subagent_type: pr-diff`) to fetch a clean change set. Pass through the arg verbatim. The agent returns PR metadata (when applicable), linked issues, file stats, and the diff (full if under threshold, excerpt + per-file fetch instructions otherwise). Read the returned diff; for files where you need surrounding context, open them via `Read`. If the working tree is dirty (no-arg case), note it in the report.
- **`<path>` arg** -- review that file or directory in full (not just the diff). No `pr-diff` delegation; read the files directly.

The `pr-diff` agent already applies default exclusions (lockfiles, generated code, build output). For survey mode (path arg), exclude `target/`, generated bindings (typically under `src/bindings.rs` or `OUT_DIR`), and `Cargo.lock` yourself.

## Routing to specialist subagents

Read each changed hunk and classify. If a hunk touches:

- **`async fn`, `.await`, `tokio::`, `select!`, `JoinSet`, `Stream`, `Pin`, `Future`, or any spawning** -- this is rust-async territory. Either invoke the `rust-async` subagent for hunks involving non-trivial async correctness (cancellation, pinning, Send/Sync footguns), or apply its principles inline if the issue is obvious.
- **`unsafe {`, raw pointers (`*const`, `*mut`), `MaybeUninit`, manual `Send`/`Sync` impl, `transmute`, custom `Drop`** -- this is rust-unsafe territory. Almost always delegate to the `rust-unsafe` subagent for a soundness opinion; never wave through unsafe code on your own.
- **`extern "C"`, `#[repr(C/transparent/packed)]`, `bindgen`, `cbindgen`, `cxx::bridge`, `pyo3`, `napi`, `uniffi`, `Box::into_raw`, `CString`/`CStr`** -- this is rust-ffi territory. Delegate for soundness and ABI questions.
- **`wasm_bindgen`, `JsValue`, `web_sys`, `js_sys`, `wasm-pack` config, WASI/`wit-bindgen`** -- this is rust-wasm territory. Delegate for boundary design and size questions.
- **`axum::`, `tower::`, `hyper::`, `sqlx::`, `tracing::`, handler functions, middleware, `IntoResponse`** -- this is rust-backend territory. Delegate for architectural questions; apply inline for obvious issues.

**Delegation guidance**: invoke a subagent when you'd otherwise have to read several files of context or run tools (`cargo expand`, `cargo miri`, etc.) to give a confident opinion. For clear-cut issues that map to the rule file, just flag them inline.

When delegating, send the agent a self-contained prompt: the relevant snippet, the question, the surrounding context the agent needs. Don't make the agent re-derive what you already know.

## What to review

You are looking for issues a strong human reviewer would catch but `rustc`/`clippy` would not. Categories, roughly ordered by how often they actually matter:

1. **Ownership and borrowing** -- unnecessary `.clone()`, `String` parameters where `&str`/`impl AsRef<str>` would do, `Vec<T>` where `&[T]` would do, lifetime annotations that could be elided or that hide a deeper problem, references to local variables across function returns.
2. **Error handling** -- `unwrap()`/`expect()` in non-prototype code, `Box<dyn Error>` in library code (should be a typed error), inconsistent error vocabulary (mixing `anyhow` and `thiserror` confusingly), error messages that leak internals, missing `?` propagation, `Result<T, ()>` that loses information.
3. **API design** -- builders that should be type-state, types that should be newtype-wrapped (especially primitive IDs and units), missing `#[must_use]`, public types that should be sealed, getter conventions (`field()` not `get_field()`), constructor patterns.
4. **Trait usage** -- `From`/`TryFrom` opportunities (vs ad-hoc `from_*` constructors), missing `AsRef`/`Borrow` to broaden APIs, `impl Trait` vs explicit trait objects, blanket impl conflicts, traits that should be sealed.
5. **Smart pointers and concurrency** -- `Rc<RefCell<T>>` where ownership could be linear, `Arc<Mutex<T>>` patterns that hide a design issue, `Mutex` that should be `RwLock` or vice versa, holding a `MutexGuard` across an `.await` (delegate to rust-async).
6. **Iterator design** -- collecting then iterating (could chain), `into_iter` vs `iter` vs `iter_mut` choices, missing `Iterator` impls that would unlock standard adapters, `collect::<Vec<_>>()` followed by `.iter()`.
7. **Async correctness** (route to rust-async) -- `select!` with cancel-unsafe branches, `std::sync::Mutex` across awaits, unbounded channels masking backpressure, detached `spawn` with no shutdown plan, `async-trait` where native `async fn` in trait works.
8. **Unsafe soundness** (route to rust-unsafe) -- any `unsafe` block without a `// SAFETY:` comment, public `unsafe fn` without a `# Safety` doc, manual pin projection, manual `Send`/`Sync`, anything that smells like aliasing violations.
9. **FFI correctness** (route to rust-ffi) -- panics that could cross an `extern "C"` boundary, non-`repr(C)` types in FFI signatures, ownership-across-boundary unclear, missing null checks on incoming pointers.
10. **WASM design** (route to rust-wasm) -- accidental tokio pull-in, per-call serialization across the JS boundary, missing `console_error_panic_hook`, unnecessary `web-sys` features inflating bundle.
11. **Module design** -- overly permissive `pub` (use `pub(crate)`/`pub(super)`), missing re-exports for ergonomic paths, modules that should be split, modules that should be merged.
12. **Cargo features** -- accidental default features, missing `default-features = false` on optional deps, features that aren't additive (cargo features must be additive), feature gates without matching `#[cfg(feature = "...")]` on items.
13. **Documentation** -- missing `# Examples`, missing `# Errors`, missing `# Panics`, missing `# Safety` on `unsafe fn`, intra-doc links that could replace ad-hoc backticks, undocumented public items.
14. **Testing gaps** -- no `#[cfg(test)]` for testable internals, integration tests that should be unit tests (or vice versa), doctests that don't compile, missing property tests for invariant-heavy code.
15. **Performance smells** -- unnecessary allocation in hot paths (`format!` vs `write!`), `.collect::<Vec<_>>()` in iterator chains that don't need materialization, `Mutex` contention patterns, `Vec::new()` then push in a loop where capacity is knowable.

## Process

Run these in parallel where possible:

1. Resolve scope (above). Capture the file list and the diff.
2. Read each changed file. For small files, read the whole thing for context; type/lifetime issues often live in surrounding code.
3. Check `Cargo.toml` and the nearest `tsconfig`-equivalent (`rustfmt.toml`, `clippy.toml`, `rust-toolchain`, `.cargo/config.toml`) for project conventions.
4. Look at the closest `CLAUDE.md` to changed files for project-specific rules.
5. Walk the rule file's categories against the diff.
6. Route specialist questions to specialist subagents in parallel where the issues are independent.

## Reporting

Findings grouped by severity:

- **blocker** -- UB risk, panic on common input, soundness violation, security issue (timing-attack-prone comparison, missing input validation at trust boundary), correctness bug.
- **major** -- significant API or design issue (ownership model that will hurt later, error type that loses information, generic that should be a trait object, missing typestate).
- **minor** -- idiom/style with clear fixes (`String` where `&str` would do, missing `#[must_use]`, builder where typestate would help).
- **nit** -- naming, doc, micro-style.

Format:

```
**[severity]** `path/to/file.rs:LINE` -- short headline

<one or two sentence explanation>

<optional: suggested fix as a code snippet or one-line description>
```

For subagent-delegated findings, prefix the headline with the subagent name in brackets: `**[major]** [rust-async] src/server.rs:42 -- MutexGuard held across .await`.

Open with: `Reviewed N files, M findings (X blockers, Y major, Z minor, W nits). Routed K hunks to specialist subagents.`

If the change is small or clean, "No findings worth flagging" is an honest answer.

## What NOT to do

- **Do not** re-report rustc/clippy output.
- **Do not** post comments to GitHub. This skill only reports to chat.
- **Do not** rewrite code. Suggest fixes inline.
- **Do not** invoke a subagent for trivial issues -- only when delegation actually saves context or buys expertise.
- **Do not** apply the rule file dogmatically. The user's judgment and the local codebase conventions win. If the project does something the rule discourages, mention it once.
- **Do not** flag things explicitly required by a project's `CLAUDE.md`.
- **Do not** put backlinks or sources in the report.

## Quick decision references

- **`thiserror` vs `anyhow`**: `thiserror` for library / typed errors that callers might match on; `anyhow` for applications/binaries / errors that just need context and propagation. Mixing both in one crate is fine if the boundary is clear (library exports `thiserror`, binary wraps with `anyhow`).
- **`Mutex` vs `RwLock`**: `RwLock` only when read contention is real and reads dominate writes; otherwise `Mutex` (often faster due to simpler locking).
- **`Arc<Mutex<T>>` vs message-passing**: prefer channels for inter-task communication; reach for shared mutex only when state is genuinely shared and access is brief.
- **`impl Trait` vs `Box<dyn Trait>` vs generic**: `impl Trait` for return-position when concrete and static dispatch; generic for parameters; `Box<dyn Trait>` for heterogeneous collections or trait objects through ABI boundaries.
- **`Send + Sync` bounds**: only add what you need; `Send + Sync` is the maximally restrictive default but unnecessary for many libraries.
