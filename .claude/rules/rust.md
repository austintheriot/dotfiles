---
paths:
  - "**/*.rs"
---

# Rust

Applies when editing or writing Rust. Assumes language fluency; targets review-worthy idioms, not what `rustc` already catches. Distilled from *Effective Rust* (Drysdale), the Rust API Guidelines, and ecosystem consensus.

## Types carry proofs

- **Make illegal states unrepresentable.** Algebraic types are the language's main expressive lever. Watch for: `bool` arguments toggling behavior, parallel `Option<T>` fields that should be one enum, "only valid if that flag is set" comments.
- **Use newtypes, not type aliases, for distinct units.** `struct UserId(u64)` and `struct OrderId(u64)` cannot be swapped; `type UserId = u64;` gives zero safety. Newtypes also dodge the orphan rule and seal invariants -- only the constructor builds a `ValidatedEmail`.
- **Parse, don't validate, at boundaries.** Convert untrusted shapes into refined types once; pass the refined type. A function that takes `&str` and `panic!`s on bad input is a smell; return `Result<Refined, ParseError>` so the proof travels with the value.

## Ownership and borrowing

- **Take `&str` and `&[T]`, not `String` or `Vec<T>`, in arguments unless you need ownership.** `&String` and `&Vec<T>` are almost always wrong: strictly less flexible than `&str` / `&[T]`. Use `impl AsRef<Path>` (or `AsRef<str>`) when the conversion is cheap and being maximally accepting helps.
- **Return owned types; borrow in arguments.** A function with no input lifetime tied to its `&str` return is a design problem; either return `String`, or accept `&str` and elide.
- **Lifetime elision is good; named lifetimes carry meaning.** Add a name (`'src`, `'arena`) when the relationship between multiple references matters. Don't sprinkle `'a` to satisfy the compiler -- if you're fighting it with lifetimes, the fix is usually to clone, restructure ownership, or use an arena.
- **`.clone()` knowingly, not reflexively.** Cloning is often right. Cloning inside a hot loop, or on a `Vec<String>` where a slice would do, is a review flag.

## Error handling

- **Libraries: typed error enum with `thiserror`.** Callers match on variants; `Box<dyn Error>` strips that. Use `#[from]` so `?` works. `Error` already requires `Display` and `Debug`.
- **Applications: `anyhow::Result` plus `.context(...)`.** Top of the stack only needs a chain of human context. Watch for `anyhow` leaking into a library's public API.
- **`?` over `match` for propagation.** Explicit `match` is fine when you genuinely branch on the error. `if let Err(_) = ... { return ... }` is a smell.
- **`unwrap()`/`expect()` outside prototypes needs justification.** Prefer `expect("invariant: ...")` over bare `unwrap()`; the invariant message is the justification.

## Traits and generics

- **`From`/`Into` for infallible conversions, `TryFrom`/`TryInto` for fallible.** Implement `From`; you get `Into` free. Method prefixes encode cost: `as_*` free borrow-to-borrow, `to_*` does work, `into_*` consumes.
- **`AsRef<T>` for cheap reference conversion; `Borrow<T>` only when you need `Hash`/`Eq`/`Ord` equivalence between borrowed and owned forms** (why `HashMap::get` takes `&str` for a `String` key). Don't reach for `Borrow` when `AsRef` suffices.
- **Generics vs `dyn Trait`, deliberately.** Generics monomorphize: fast call, bigger binary, can carry multiple bounds and associated types. `dyn Trait` is one function, vtable dispatch, must be object-safe (no generic methods, no `Self` by value, no associated constants). Heterogeneous collections need `dyn`; otherwise prefer generics.
- **Sealed traits when you publish but don't want downstream impls.** `pub trait Foo: private::Sealed {}` with a private supertrait lets you add methods later without breaking users.
- **`impl Trait` in return position hides the concrete type; in argument position it's a sugar generic.** Use it when the type is unnameable (closures, opaque iterators). Avoid in public APIs where callers may want to store the type; return a named type or a boxed trait object instead.
- **Don't define a trait that has one implementor.** Just use the type. Traits earn their keep with multiple impls, polymorphic callers, or a generic test seam.

## Smart pointers and shared state

- **`Box<T>` for heap and trait objects; `Rc<T>` for single-threaded shared ownership; `Arc<T>` cross-thread.** `Rc<Mutex<T>>` is a bug -- cross-thread needs `Arc`; single-thread needs `RefCell`, not `Mutex`. `Arc<Mutex<T>>` is the canonical shared-mutable pattern.
- **`RefCell`/`Cell` for single-threaded interior mutability; `Mutex`/`RwLock` multi-threaded.** `Cell<T>` when `T: Copy` -- no runtime borrow-check overhead. `RefCell` panics on aliased borrow; that's a logic bug, not recoverable.
- **Never hold a `MutexGuard` across `.await`.** Not `Send`, and when it compiles you've serialized the runtime. Drop the guard first, or restructure.

## Collections and iterators

- **`Vec<T>` is the default. `VecDeque` only when you need cheap front-pop. `HashMap` for unordered O(1), `BTreeMap` for ordered iteration or range queries.** `HashMap` randomizes iteration order by design -- depending on it is a bug.
- **Iterator chains over manual loops.** A `for` loop pushing into a fresh `Vec` is `.map().collect()`. `collect::<Result<Vec<_>, _>>()` short-circuits on the first error -- the canonical "process all or fail."
- **Don't materialize when you could iterate.** Return `impl Iterator<Item = ...>` instead of `Vec<...>` so the caller decides whether to collect, count, or short-circuit. Especially in library APIs.

## Naming and API shape

- **Getters are bare nouns, not `get_foo`.** `self.name()`. `_mut` attaches to the return type: `as_mut_slice`, not `as_slice_mut`. `get_*` is reserved for the single-obvious-thing case (`Cell::get`).
- **Iterator methods: `iter`, `iter_mut`, `into_iter`. Iterator types match: `Iter`, `IterMut`, `IntoIter`.**
- **Constructors are inherent: `new`, `with_<detail>`, `from_<source>`.** Builders for anything with optional knobs or more than 2-3 params.
- **`UpperCamelCase` types (`Uuid`, not `UUID`); `snake_case` values; `SCREAMING_SNAKE_CASE` consts.** A single-letter "word" mid-identifier is wrong (`btree_map`, not `b_tree_map`); trailing is fine (`PI_2`).
- **Take meaningful types, not `bool` or `Option<T>` as flags.** `open(path, true, false)` is invisible -- make an enum.

## Modules, visibility, features

- **Default to private; use `pub(crate)` for crate-internal, `pub(super)` for parent-only.** Anything `pub` is a semver commitment. `pub use` to re-export public surface from the crate root.
- **Struct fields default private with accessors.** Public fields are permanent; newtype-wrap to recover later without a major bump.
- **Cargo features are additive only.** `--no-default-features` plus any subset must compile. A feature that *disables* behavior is a bug; split crates or move the default.
- **Re-export dependency types that appear in your public API** (`pub use serde::Deserialize`) so downstream users don't pin the same version separately.

## Macros

- **Don't write a macro if a function or generic suffices.** Macros break go-to-definition, complicate errors, resist refactoring. The bar is "structural boilerplate a function can't express."
- **`macro_rules!` for shape-matching; proc macros only when you genuinely need to parse Rust syntax** (custom derives, attribute macros). Proc macros are a separate crate, a build cost, and a debuggability tax.
- **Always derive the standard traits where they make sense.** `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`. Public types must implement `Debug`. Don't hand-roll what `derive` gives.

## Testing and documentation

- **Unit tests in `#[cfg(test)] mod tests` at the bottom of the file under test; integration tests in `tests/`.** Doctests in `///` double as examples and run under `cargo test`. Use `?` in doctests, never `unwrap`.
- **Every public item gets `# Examples`,** plus `# Errors` for `Result`-returning functions, `# Panics` for anything that can panic, `# Safety` for `unsafe fn`. Intra-doc links (`` [`MyType`] ``) over URLs -- they survive renames.
- **`proptest` or `quickcheck` for invariants over algorithms** (round-trip serialization, sort stability, parser/printer equivalence). One property test beats a dozen examples.

## Clippy lints worth respecting

`needless_clone`, `redundant_clone`, `needless_collect`, `unnecessary_wraps`, `large_enum_variant`, `match_like_matches_macro`, `option_if_let_else`, `or_fun_call` (eager arg in `unwrap_or(expensive())`), `single_match` (use `if let`), `wildcard_imports`. Run `cargo clippy --all-targets -- -D warnings` in CI; allows are documented exceptions.

## Anti-patterns reviewers should flag

`String` / `Vec<T>` arguments where `&str` / `&[T]` works; `&String` or `&Vec<T>` ever; reflexive `.clone()` to silence the borrow checker; `unwrap()` outside tests/prototypes; `Box<dyn Error>` in a library's public surface; a trait with one implementor; `match` on `Result` purely to propagate when `?` suffices; holding a `MutexGuard` across `.await`; `bool` parameters whose meaning isn't obvious at the call site; `if let Some(x) = opt { x } else { default }` instead of `opt.unwrap_or(default)`; collecting into a `Vec` only to immediately iterate again; type aliases used where a newtype was wanted.
