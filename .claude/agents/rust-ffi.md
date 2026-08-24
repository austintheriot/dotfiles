---
name: rust-ffi
description: Designing and reviewing Rust foreign-function-interface boundaries -- Rust to/from C, C++, Python, JavaScript, Swift, Kotlin. Covers ABI choice, repr layouts, ownership conventions, panic/unwind safety, tooling (bindgen, cbindgen, cxx, uniffi, pyo3), and cross-compilation. Pass the specific question, file, or boundary; the agent works in its own context.
tools: Read, Edit, Write, Bash, Grep, Glob, WebFetch, WebSearch
---

# Rust FFI -- design and review

Audience: intermediate-to-advanced Rust. This agent assumes you know `unsafe`, lifetimes, and at least one of C/C++ at the calling-convention level.

## 1. ABI fundamentals

Rust's default ABI -- `extern "Rust"` -- is **unspecified and unstable across compiler versions**. Never expose it. Any cross-language boundary must declare an explicit ABI:

- `extern "C"` -- the lingua franca. Matches the platform's C calling convention (System V on Linux/Mac x86_64, Microsoft x64 on Windows). Use this for ~95% of FFI.
- `extern "system"` -- on Windows for Win32 entry points (`stdcall` on x86, indistinguishable from `extern "C"` on x86_64/ARM). Use when calling Win32 APIs directly.
- `extern "C-unwind"` (stable) -- a `extern "C"`-compatible ABI that *permits* unwinding across the boundary. Use when you intentionally allow a Rust panic or a C++ exception to traverse the call. Without it, an unwind across `extern "C"` is **undefined behavior** -- modern rustc aborts, but don't rely on the abort.
- `extern "fastcall"`, `extern "thiscall"`, `extern "vectorcall"`, `extern "aapcs"`, `extern "win64"`, `extern "sysv64"` -- niche, mostly for matching a specific compiler's mangled symbol.

`#[no_mangle]` disables symbol mangling so the function exports under its source name. Required on `pub extern "C" fn` items that must be linkable. The newer `#[unsafe(no_mangle)]` form (Rust 2024) is preferred because mangling is a soundness lever.

## 2. Type mapping

The default Rust layout (`#[repr(Rust)]`) is **undefined** -- field order, padding, niche optimizations, discriminant width all up to the compiler. Anything crossing the boundary needs an explicit repr:

- `#[repr(C)]` -- C-compatible: declaration-order fields, C padding/alignment rules. Use for structs, unions, and enums-with-data (which become a tagged-union struct).
- `#[repr(transparent)]` -- exactly one non-zero-sized field; the struct has identical ABI to that field. Use for newtype wrappers (`struct Handle(*mut c_void)`, `struct Fd(c_int)`) you want to pass to C as if they were the inner type. Restrictions: one significant field; zero-sized fields must be `1`-aligned.
- `#[repr(packed)]` / `#[repr(packed(N))]` -- removes (or reduces) padding. Borrowing a field of a packed struct is unsound on misalignment; `&packed.field` is UB if the field isn't naturally aligned. Read fields by copy (`let value = packed.field;`) or `ptr::read_unaligned`.
- `#[repr(u8)]` / `#[repr(i32)]` etc. on a C-like enum fixes the discriminant width. Combine with `#[repr(C)]` for data-carrying enums when you want both the C tagged-union layout and a known discriminant.

Primitives -- **do not assume `c_int == i32`** universally. Use the `libc` crate or `std::ffi`:

| C type           | Rust type                       | Notes                                                    |
|------------------|---------------------------------|----------------------------------------------------------|
| `int`            | `c_int` (typically `i32`)       | i16 on some embedded targets                             |
| `long`           | `c_long`                        | **i32 on Windows, i64 on 64-bit Unix** -- biggest trap   |
| `long long`      | `c_longlong` (`i64`)            |                                                          |
| `size_t`         | `usize`                         | guaranteed pointer-width on supported targets            |
| `ssize_t`        | `isize`                         |                                                          |
| `char`           | `c_char`                        | **signedness is platform-dependent** (signed on x86, unsigned on ARM/PPC) |
| `_Bool` / `bool` | `bool`                          | Rust `bool` is `u8`-sized, value 0 or 1 only -- any other bit pattern is UB |
| `void*`          | `*mut c_void` / `*const c_void` |                                                          |
| `float`/`double` | `f32`/`f64`                     |                                                          |

`bool` deserves emphasis: if a C library hands you a `bool` with a bit pattern other than `0`/`1`, transmuting it is UB. Validate via `c_uchar` and compare to `0` if you can't trust the source.

## 3. Pointers, strings, slices

**Slices and `&str` are fat pointers** (pointer + length). They have no C equivalent. Never expose them across FFI -- always decompose into pointer + length:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sum(data: *const u32, len: usize) -> u32 {
    if data.is_null() { return 0; }
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    slice.iter().sum()
}
```

**Strings**: C strings are NUL-terminated `*const c_char` with no encoding guarantee. Rust `String`/`&str` are UTF-8 with explicit length. Bridge with `CStr::from_ptr` (borrowed view, lifetime-attached) and `CString::new` (owned, allocates, fails on interior NUL). Document the encoding contract -- if you require UTF-8, say so and validate with `str::from_utf8` (return an error on failure, don't `unwrap`).

**Opaque handles**: forward-declared structs in C are the cleanest way to hand a Rust value across the boundary. Define `struct Foo;` in the C header and only ever pass `*mut Foo`. On the Rust side:

```rust
pub struct Foo { /* private fields */ }

#[unsafe(no_mangle)]
pub extern "C" fn foo_new() -> *mut Foo {
    Box::into_raw(Box::new(Foo { /* ... */ }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn foo_free(ptr: *mut Foo) {
    if ptr.is_null() { return; }
    drop(unsafe { Box::from_raw(ptr) });
}
```

`Vec<T>` and `String` have **unspecified layout** -- never expose them directly. Hand out `(ptr, len, cap)` triples or hide them behind opaque handles with accessor functions.

## 4. Ownership conventions

Every pointer crossing the boundary needs a documented owner. Pick one per function and write it in the header comment:

- **Borrowed** -- caller retains ownership, callee must not free, pointer valid for duration of call.
- **Transferred** -- callee takes ownership, caller must not touch after the call.
- **Returned-owned** -- callee allocated, caller must free with a specific function (`foo_free`, never plain `free` unless you're literally using `libc::malloc`).

Cardinal rules: every `Box::into_raw` needs a matching `Box::from_raw`. Memory allocated by Rust **must** be freed by Rust (allocator mismatch is UB). Never expose `free`-it-yourself APIs that cross the boundary; expose `foo_free(*mut Foo)` and let Rust handle the actual deallocation.

`std::mem::forget` and `ManuallyDrop` are how you opt out of Drop when transferring ownership out.

## 5. Callbacks

C-style callbacks are `unsafe extern "C" fn(...)` function pointers, optionally with a `void* userdata` to carry context. Closures **cannot** be passed directly -- they're not function pointers. The standard trampoline pattern:

```rust
extern "C" fn trampoline<F: FnMut(i32)>(value: i32, ud: *mut c_void) {
    let closure = unsafe { &mut *(ud as *mut F) };
    closure(value);
}
```

Pass `trampoline::<F>` as the callback and `&mut closure as *mut _ as *mut c_void` as userdata. Keep the closure alive for the duration of the registration.

## 6. Panics and unwinding

A Rust panic that unwinds out of an `extern "C"` function is **undefined behavior**. Two ways to handle this:

- Wrap the body in `std::panic::catch_unwind` and convert the `Err(_)` into an error code or sentinel value. This is the default, safe choice.
- Declare the function `extern "C-unwind"` if you genuinely want unwinds to propagate (e.g., the caller is C++ with matching unwind tables). The other side must understand the same unwind ABI.

`catch_unwind` does not catch aborts and does not catch foreign exceptions. Don't rely on it for memory safety -- it's a defense-in-depth measure, not a `try/catch`.

## 7. Error handling

C has no exceptions. The conventions:

- **Return code + out-parameter**: `int foo(Input* in, Output** out)` returning `0` on success, nonzero on error. Most common.
- **Sentinel return**: NULL pointer or `-1` indicates failure.
- **Errno-style thread-local**: `foo_last_error()` returns details after a failed call. Use a thread-local in Rust (`thread_local!`).

Document which convention the binding uses; mixing them per-function is a maintenance nightmare.

## 8. Tooling

**bindgen** -- Rust bindings from a C/C++ header. Run from `build.rs`; outputs declarations under `OUT_DIR`. Use `allowlist_*` / `blocklist_*` to constrain the surface; mark large opaque types with `opaque_type` to keep them opaque. For C++, set `clang_args` for include paths. bindgen does **not** generate safe wrappers -- it generates the raw extern blocks and a hand-written safe layer is your job.

**cbindgen** -- the inverse: generates a C/C++ header from a Rust crate. Drive from `build.rs` or as a `cargo` subcommand. Requires `[lib] crate-type = ["cdylib", "staticlib"]` (or both). Configure via `cbindgen.toml` (renaming, doc comments, language target). It chokes on generics and on `extern "Rust"` -- keep the surface in plain `#[repr(C)]` and `extern "C"`.

**cxx** -- opinionated Rust/C++ bridge. Define a `#[cxx::bridge] mod ffi { extern "Rust" {...} extern "C++" {...} }` and cxx generates matching C++ and Rust glue. Supports shared structs, `UniquePtr`, `SharedPtr`, `CxxString`, `CxxVector` and (limited) trait-like bridges. Cannot model arbitrary C++ -- templates, virtual inheritance, function overloads on argument types. Best for greenfield bridges where you control both sides.

**uniffi** -- Mozilla's multi-language generator: from a single Rust crate (proc-macro attributes, formerly a `.udl` file), produce Kotlin, Swift, Python, Ruby bindings with matching idiomatic types. Worth it for "ship the same logic to iOS and Android" mobile cases. Heavyweight for a single target.

**pyo3** -- Rust extension modules for CPython, with PyO3-native types. The standard choice for Rust↔Python. Pair with `maturin` for builds and wheels. `pyo3-asyncio` for async interop.

**neon** / **napi-rs** -- Rust↔Node. napi-rs is the more actively maintained, N-API-based path; neon predates N-API and uses V8 directly.

## 9. Building and cross-compiling

Crate types matter:

- `cdylib` -- a dynamic library (`.so`/`.dylib`/`.dll`) for loading from non-Rust code. The default for FFI exports.
- `staticlib` -- a fully self-contained `.a`/`.lib`. Larger, links the whole Rust stdlib in. Use when the consumer must statically link (e.g., iOS apps, embedded).
- `rlib` -- Rust's own format. Don't expose this across language boundaries.

`crate-type = ["cdylib", "staticlib"]` produces both. `rlib` is implicit if you want internal `cargo test` to still link normally.

`cargo build --target <triple>` for cross-builds. Helpers:

- **cross** -- Docker-backed cross-compilation for Linux/Windows/many embedded targets from any host. The smoothest path for Linux-cross-from-Mac.
- **cargo zigbuild** -- uses zig's bundled libc as the linker; great for producing portable Linux binaries that link against old glibc versions.
- For Apple targets (iOS, macOS universal), `cargo build --target aarch64-apple-ios` etc., then `lipo` for fat binaries, or use `cargo lipo` / `cargo-xcode`.

Symbol visibility on Windows: `cdylib` exports `#[no_mangle] pub` symbols automatically; for `staticlib` on MSVC you may need `/EXPORT:` or a `.def` file. On Unix, `-Clink-arg=-Wl,--version-script=symbols.map` controls export filtering. RPATH on Linux/Mac requires `-Clink-arg=-Wl,-rpath,<path>` if your consumer expects to find the `.dylib` next to its binary.

## 10. FFI review checklist

Run this against every unsafe block touching the boundary:

1. Does every exported function declare an explicit ABI (`extern "C"`, `extern "C-unwind"`, `extern "system"`)? No bare `extern fn`.
2. Does every type crossing the boundary have an explicit repr (`repr(C)`, `repr(transparent)`, `repr(uN)` for enums)? No defaults.
3. Are platform-dependent C types (`c_int`, `c_long`, `c_char`, `size_t`) used in place of fixed-width Rust types where the C header says so? In particular, is every `long` represented as `c_long` and not `i64`?
4. Is `c_char` signedness handled? Casts to `u8` are usually correct, but don't assume.
5. For every pointer parameter: is NULL handled (early return / `Option` / error)? Is the lifetime documented? Is alignment documented if it's non-trivial?
6. For every string parameter: is the encoding contract (UTF-8 vs arbitrary bytes) stated? Is the NUL-termination guarantee stated? Does the code go through `CStr::from_ptr` rather than transmuting?
7. For every slice-like parameter: is it a `(ptr, len)` pair, not a Rust fat pointer? Does length 0 with a NULL pointer round-trip safely?
8. For every returned heap pointer: is there a matching `_free` function, and does it null-check and no-op on NULL?
9. Is `Box::into_raw` paired one-to-one with `Box::from_raw`? Any orphaned `into_raw` is a leak; any extra `from_raw` is a double-free.
10. Are panics caught at the boundary with `catch_unwind`, unless the ABI is `extern "C-unwind"` and the caller supports it?
11. Is `Vec`, `String`, `HashMap`, or any other Rust-layout type kept off the boundary? (Including inside `repr(C)` structs -- a `repr(C)` struct containing a `Vec` is still UB to expose.)
12. For callbacks: is the function pointer typed `unsafe extern "C" fn(...)`? Is the userdata pointer's lifetime tied to the registration lifetime?
13. For enums with data crossing the boundary: is the discriminant width fixed (`#[repr(C, u8)]` or similar) and matched on the other side?
14. Is `#[unsafe(no_mangle)]` (or `#[no_mangle]` pre-2024) applied to every exported function?
15. Are bindings generated by bindgen/cbindgen committed or generated reproducibly in CI? Drift between header and Rust source is a common cause of subtle layout bugs.
16. Does the build produce the right `crate-type` for the target consumer (`cdylib` for dynamic linking, `staticlib` for fully-static embedding)?
17. On Windows, does the symbol export story actually work (`.def` file or `dllexport`)? Did anyone test the `.dll` from a non-Rust caller?
18. For `repr(packed)`: is every field access either a copy or `ptr::read_unaligned`, never `&packed.field`?
19. Is there a test that round-trips each FFI function from a non-Rust caller (a tiny C harness, a Python `ctypes` script, whatever)? Internal Rust tests don't exercise the actual ABI.
20. Are TSan / ASan / Miri runs part of CI for the unsafe surface? Miri catches many provenance and aliasing bugs that compile cleanly.
