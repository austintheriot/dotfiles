---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Native Bridge Integration

A reference for reviewing the boundary between a cross-platform host (JavaScript, Rust, Dart, Kotlin) and native platform code: the wrapper landscape, the inter-process contract, and the trust boundary across it. Used by the `native-bridge` subagent.

Distinct from:
- **`desktop-native`** / **`mobile-native`**: platform runtime behavior. We own the boundary itself.
- **`input-and-peripherals`**: what the capabilities do and how their permissions work. We own how a request crosses the bridge to reach them.
- **`security`**: general threat model. We own the renderer-to-native trust boundary specifically.
- **`api-design`**: consumer contracts generally. We own the bridge contract, which is a contract with a shipped-binary version-skew problem.
- **`rust-ffi`** / **`webassembly`**: those specific boundaries.

The core thesis: **the bridge is a contract between two things that ship on different schedules.** Native code ships through app review; the JavaScript layer may hot-update. Every bridge contract must therefore survive version skew in both directions, and a signature change that would be a trivial refactor inside one process becomes a compatibility break across the bridge.

The operational priority: **establish the trust boundary first.** If the renderer displays any remote content, it is attacker-controlled, and every native handler is a privilege-escalation target. Validation must happen on the native side; validation in the renderer is advisory.

Verification markers: **[V]** verified against primary source, **[U]** unverified.

---

## Universal principles

### Context isolation raises cost; it is not a trust boundary

Electron's context isolation has been bypassed repeatedly -- four times in 2020, again in 2023, and three times in 2026 [V]. **CVE-2026-70601 is the one to know**: a bypass via `Function.prototype.bind` hijacking that affects applications exposing Promise-returning functions through `contextBridge` -- **which is the standard pattern for wrapping `ipcRenderer.invoke`** -- with **no application-side workaround** [V].

Design as if the renderer will eventually be compromised: minimum surface, validated inputs, no ambient authority.

### Validate the sender, not just the channel

**Electron's own security checklist item 17** requires validating `event.senderFrame` in every IPC handler with a real URL parser and a host allowlist [V]. `event.sender` is insufficient: **a cross-origin iframe shares the same `WebContents` and is indistinguishable via `sender`.**

Validate **synchronously before any `await`**, and check `frame.detached` rather than only null, since Electron 33 changed that behavior.

### The bridge is a serialization boundary with a type contract that lies

Every framework's bridge silently transforms values. Reviewing the declared types is not enough; you must know what survives.

**Electron `contextBridge`** [V]: prototypes are dropped from objects, arrays, and functions; **classes and constructors do not work**; `Error` custom properties are lost; **`Symbol` cannot cross and is silently dropped**; values are **copied and frozen**, so mutation does not propagate. Since Electron 29, passing `ipcRenderer` wholesale yields an **empty object, failing closed and silently**. Map, Set, ArrayBuffer, Date, RegExp, and getter behavior are **genuinely undocumented**, folded into one "cloneable types" row.

**Electron IPC serialization** [V]: structured clone since Electron 8, with the legacy fallback removed in 9 so non-serializable values now hard-throw. The highest-traffic silent gotcha: **`Buffer` arrives as `Uint8Array`**.

**Errors are not transparent through `handle`** -- only `message` survives [V]. Stack traces and error types are erased, which is why bridge failures are so hard to diagnose from a crash report.

### Async by default; synchronous calls block the frame

`sendSync` blocks the entire renderer, and Electron's documentation calls it a last resort [V]. A synchronous bridge call from a render loop costs a frame.

**React Native's JSI has thread affinity**: `jsi::Runtime` is **not thread-safe and is bound to the JavaScript thread** [V]. Touching any `jsi::Value` off-thread is undefined behavior whose symptom is **heap corruption far from the call site**, not a caught error. Hop with `CallInvoker::invokeAsync`.

**Flutter channel handlers run on the platform main thread**; off-main work needs a background task queue.

---

## Wrapper landscape

Versions verified 2026-08-26 [V]: Electron 44.0.0 (Chromium 152, Node 24.18.1), Tauri 2.11.5, React Native 0.87.0, Expo SDK 56, Flutter 3.44.0, Capacitor 8.5.0, Kotlin 2.4.10, Compose Multiplatform 1.12.0, Pigeon 28.0.0.

**Cordova is not retired** [V] -- a common misconception. It has no Apache Attic entry, shipped major bumps across CLI, Android, and iOS within the last year, and patched a CVE. It is maintainer-constrained and openly recruiting, not dead.

### Electron

Majors every 8 weeks tracking Chromium, **supporting only the latest three**. Electron 44 requires macOS 13 or later.

**Security defaults with version attribution** [V]: `nodeIntegration` false (v5), `contextIsolation` true (v12), `sandbox` true (v20), `webSecurity` true.

Non-obvious rules from the official checklist:
- **`contextIsolation` is required even with `nodeIntegration: false`.** They are not substitutes.
- **Disabling context isolation also disables sandboxing** -- one flag revokes two protections.
- **`setPermissionRequestHandler` auto-approves everything by default**, so camera, microphone, and geolocation are silently granted unless a handler is installed.
- **Exposing `ipcRenderer.on` leaks `IpcRendererEvent.sender`.** Both the raw method and the channel-scoped-but-raw-callback form are wrong; only wrapping the callback and dropping the event argument is correct.

**Fuses**: the four dangerous ones -- `runAsNode`, `nodeCliInspect`, `nodeOptions`, `grantFileProtocolExtraPrivileges` -- are **enabled by default** [V]. ASAR integrity validation is **disabled by default and is macOS and Windows only, never Linux**. Three separate ASAR-integrity bypasses have each defeated exactly the fuses meant to stop them.

**`BrowserView` was deprecated in Electron 30** in favor of `BaseWindow` plus `WebContentsView`; the `remote` module was **removed in v14**.

**Node-API ABI guarantees cover only `node_api.h`**, not libuv, V8, or Node's C++ APIs. Electron 44's `NODE_MODULE_VERSION` is **149 while Node 24's is 137**, which is why a Node-24-built module fails in Electron 44 **despite Electron 44 bundling Node 24.18.1** [V]. Calling JavaScript from a native thread requires `napi_threadsafe_function`.

**No official bundle-size or per-renderer memory figures exist** [V]; circulating numbers are folklore. Measure rather than cite.

### Tauri v2

The **capabilities and permissions access-control list** is the headline v1-to-v2 change. Sharp edges [V]:
- **A window in more than one capability merges the security boundaries of all of them** -- additive, so a debug capability widens production.
- Files in `capabilities/` are **auto-included by default**; default-deny applies only once listed in config.
- **On Linux and Android, Tauri cannot distinguish an embedded iframe from the window itself** -- documented, and the same class as a known CVE.

Commands map camelCase to snake_case by default, cannot be `pub` in `lib.rs`, and **async commands cannot take borrowed arguments**. For performance the contract is explicit: use `tauri::ipc::Response` for large binary payloads to bypass JSON, and `tauri::ipc::Channel` for streaming.

**CVE-2026-42184** (fixed in 2.11.1) is the cleanest lesson: `is_local_url()` used `split_once('.')` and inspected only the first subdomain, so **any** attacker-controlled host passed the local-origin check on Windows and Android [V].

**Tauri has two public third-party audits** by Radically Open Security, in-repo; the v2 audit found 11 high-severity issues pre-GA including IPC callable from any origin [V]. **Electron has no comparable public third-party audit.**

**The WebView tradeoff, stated honestly**: Linux WebKitGTK is fragmented across live distributions and, per Tauri's own discussion board, degrading. Electron pays roughly 100 MB for one tested rendering target; Tauri pays a small binary for N targets, the worst of which is getting worse. **Bundle-size comparisons that ignore this compare the wrong axis.**

### React Native

The New Architecture timeline, corrected [V]: default in 0.76; **legacy frozen in 0.80**; **mandatory in 0.82 with opt-out flags silently ignored**; **removal began in 0.84**, which also made Hermes V1 the default. **Note that Hermes V1 explicitly excludes static compilation and JIT** -- "Static Hermes" is not what shipped. The support window is narrow: as of 0.87, **0.84 is already unsupported**. The migration floor is to stop at 0.81 or Expo SDK 54 before 0.82.

**Codegen's type table is the reviewable artifact.** Its traps [V]: **`number` cannot be nullable** because it maps to a primitive `double`, so "no value" gets faked as `-1` or `0` and **the TypeScript type lies**; type unions work only as callback parameters; and bare `Object` degrades to an untyped map -- typed-looking with zero enforcement.

**Swift still needs an Objective-C++ shim** for module authoring, since Swift cannot conform to a C++ protocol. This is the largest ergonomic gap against **Expo Modules**, whose declarative API needs no spec file, no Codegen, and no shim.

**Version skew**: **App Center retired 2025-03-31, taking CodePush with it**, and the open-sourced server repositories are archived and read-only [V]. On EAS Update, **`runtimeVersion: fingerprint` is correct because it hashes actual native state**; the `appVersion` policy is the footgun, since developers forget to bump it when adding a native dependency and the over-the-air update lands on a binary lacking the module.

### Flutter

`StandardMessageCodec`'s **integer-width split is a latent bug generator** [V]: Dart integers of 32 bits or fewer arrive as one native type and larger ones as another, so native code breaks on **value magnitude, not type**.

**Channel names are bare global strings with no namespacing enforcement** -- two plugins registering the same name silently overwrite each other, last one winning, with no error [V].

**Pigeon** adds custom classes, nested types, and enums over the raw codec, but **both sides must use the same Pigeon version or you get crashes** -- a concrete instance of bridge version skew. Swift's error class is `PigeonError`, not `FlutterError`.

**Impeller has replaced Skia**: iOS has no opt-out, and Flutter 3.44 **removed the Skia backend for Android 10 and later with no fallback flag** [V]. Advice to "disable Impeller" is obsolete.

### Kotlin Multiplatform

**Swift Export is the story**, and it is more capable than its announcement suggested [V]: multi-module projects map to separate Swift modules, **nullability works without boxing primitives**, suspend functions become `try await`, and **`Flow` maps to `AsyncSequence` natively**. Kotlin 2.4.20-RC2 adds sealed classes to Swift enums with exhaustive switching. **This is close to obsoleting SKIE.** The remaining gap: generics are type-erased to upper bounds.

Compose Multiplatform iOS and desktop are stable; **web and WebAssembly are beta**, not alpha.

### WebView in native

**WKWebView**: prefer **`callAsyncJavaScript(_:arguments:in:in:completionHandler:)` over `evaluateJavaScript`** [V]. It takes a real arguments dictionary and supports `await`, whereas `evaluateJavaScript` forces string interpolation into JavaScript source -- **a script-injection bug the moment any interpolated value is attacker-influenced.** Watch the canonical retain cycle through `WKUserContentController`, and remove script message handlers explicitly.

**Android WebView**, with Google's own ranking [V]:

| | `addWebMessageListener` | `postWebMessage` | `addJavascriptInterface` |
|---|---|---|---|
| Security | **Highest** (origin allowlist) | High | **Low, no origin checks** |
| Thread | Main | async | **Background** |
| Recommended | **Yes** | No | **No** |

`addJavascriptInterface` callbacks run on a **background thread** (touching views crashes), are **exposed to every frame including iframes**, and **`WebView.getUrl()` is documented as unreliable**, so **origin validation cannot be retrofitted onto it**. `addWebMessageListener` provides `sourceOrigin` and `isMainFrame`. Origin rules are exact: `https://*.example.com` matches subdomains but **not the apex** -- list both. Google is candid that origin rules stop third-party injection, **not stored cross-site scripting on your own domain**.

**Local content** should use `WebViewAssetLoader` over an HTTPS-scheme virtual host. `file://` and `data:` are **opaque origins that cannot use fetch**, which is exactly what drives teams to enable universal file access -- the classic local-file exfiltration hole. Set both file-URL flags false on all API levels.

### Capacitor

Registration moved off Objective-C runtime macros to an explicit method array, so **a method you forget to list simply does not exist to JavaScript** [V]. Defaults use custom schemes rather than a local HTTP server, with `androidScheme: "https"` deliberate to preserve a secure context so local storage, service workers, and Web Crypto behave.

---

## Standards positions: hardware APIs are Chromium-only by policy

Queried from the vendor position databases directly rather than blog summaries [V]:

- **Mozilla is `negative` on Web Bluetooth, WebUSB, WebHID, Web NFC, File System Access, and Generic Sensor.**
- **WebKit `opposes` Web Bluetooth, WebUSB, Web Serial, File System Access, and Geolocation Sensor.**

**The consequence is architectural**: these are Chromium-only **by vendor policy, not by lag**, and because iOS forces WKWebView, **Chrome on iOS cannot ship them either**. Any design betting on "Safari will catch up" is betting against a written position. Note also that `webview_android` is false for Web Bluetooth, so **Cordova and Capacitor wrappers cannot use it at all** [V].

---

## Anti-pattern catalog

### Trust boundary
- `nodeIntegration: true` with any remote content.
- `contextIsolation: false`, which also disables sandboxing.
- No `setPermissionRequestHandler`, silently granting camera, microphone, and location.
- IPC handlers that do not validate `event.senderFrame` against a host allowlist, or that validate after an `await`.
- Exposing `ipcRenderer` or a raw `ipcRenderer.on` callback through `contextBridge`.
- Electron fuses left at their dangerous defaults; ASAR integrity left off.
- Treating ASAR as encryption.
- `addJavascriptInterface` in any new Android code, or attempting to retrofit origin checks onto it.
- Origin rules listing the wildcard subdomain but not the apex.
- Universal file access enabled to work around opaque `file://` origins.
- String interpolation into `evaluateJavaScript`.
- Tauri capabilities with a wildcard window list, or stray files in `capabilities/`.

### Contract and serialization
- Assuming a class, prototype, `Symbol`, or custom `Error` property survives `contextBridge`.
- Assuming `Buffer` arrives as `Buffer` rather than `Uint8Array`.
- Relying on error type or stack across `handle`.
- React Native Codegen using `number` to mean optional, which the type cannot express.
- Bare `Object` in a Codegen spec, which is typed-looking with no enforcement.
- Flutter channel names without namespacing, silently colliding across plugins.
- Pigeon version mismatch across packages.
- Native modules built against Node's ABI rather than Electron's.

### Threading and performance
- Touching a `jsi::Value` off the JavaScript thread, producing heap corruption far from the call site.
- `sendSync` on a render path.
- Per-frame chatty bridge calls where one batched call would do.
- Large binary payloads through JSON rather than the framework's binary path.
- Flutter platform-channel work on the main thread without a background queue.

### Version skew
- Over-the-air updates keyed to application version rather than a native-state fingerprint.
- Assuming CodePush still exists.
- Targeting a React Native version past the removal boundary without migrating.
- Bridge contract changes shipped in the JavaScript layer without a native compatibility path.

---

## Schools of thought (preserve disagreement)

- **Electron versus Tauri.** Electron buys one tested rendering target for roughly 100 MB; Tauri buys a small binary and inherits N system webviews, the worst of which is degrading. **Size comparisons that ignore the fragmentation axis are measuring the wrong thing.** Tauri has public third-party audits; Electron does not.
- **React Native versus Flutter versus native.** Genuinely unsettled and mostly a hiring and ecosystem question rather than a technical one.
- **Expo Modules versus bare React Native module authoring.** Expo's declarative API removes the spec file, Codegen, and the Objective-C++ shim; the counterargument is dependency on Expo's release cadence.
- **How much logic belongs in the shared layer.** KMP shared-logic-only preserves platform feel; Compose Multiplatform extends to shared UI and trades some of it.
- **Whether context isolation is a security boundary.** The defensible position given the bypass history is that **it raises attacker cost and is not a boundary**, so defense in depth is required regardless.

---

## What is NOT a native-bridge finding

- Platform runtime behavior on either side. Route to `desktop-native` or `mobile-native`.
- What a capability does and how its permission model works. Route to `input-and-peripherals`.
- General threat modeling and cryptography. Route to `security`.
- Rust-to-C FFI or WebAssembly boundaries specifically. Route to `rust-ffi` or `webassembly`.
- Packaging, signing, and update delivery. Route to `platform-release`.
- Generic "switch to Tauri" or "adopt Expo" advocacy without a named defect.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: `nodeIntegration` or `contextIsolation` misconfigured with remote content; an IPC handler with no sender validation reachable from renderer-controlled content; `addJavascriptInterface` exposed to remote content; string interpolation into `evaluateJavaScript` with attacker-influenced values; universal file access enabled; a `jsi::Value` touched off-thread.
- **major**: missing permission-request handler; fuses at dangerous defaults in a shipped application; ASAR integrity off where supported; Codegen `number` used as optional; Flutter channel names unnamespaced; Pigeon or native-module ABI mismatch; over-the-air runtime version keyed to application version; sender validation placed after an `await`.
- **minor**: synchronous bridge call off the hot path; chatty bridge usage that batching would fix; large payloads through JSON where a binary channel exists; relying on undocumented `contextBridge` type behavior.
- **nit**: channel naming; bridge method organization.
- **insight**: structural -- "the bridge exposes twelve fine-grained methods where three coarse ones would reduce the attack surface and the version-skew burden"; "this design assumes Web Bluetooth will reach Safari, which is a written vendor position against, not a lag"; "native and JavaScript ship on different cadences with no contract versioning, so the first skew is a support incident."

Confidence: high when the trigger is a concrete configuration flag, handler, or type declaration; medium when reasoned from architecture.

---

## Process for the native-bridge agent

1. **Identify the shell and version.** Support windows are narrow, and several frameworks have hard migration boundaries.
2. **Establish the trust boundary.** Does the renderer display remote content, load remote scripts, or render user-supplied HTML? If yes, every handler is a privilege-escalation target.
3. **Walk the security configuration**: isolation, sandbox, permission handlers, fuses, integrity, capability scope.
4. **Walk every IPC handler**: sender validation and its placement relative to `await`, input validation, and whether the operation is one the renderer should be able to request at all.
5. **Walk the type contract**: what actually survives serialization, and where the declared types lie.
6. **Walk threading**: which thread each handler runs on, and whether any runtime object crosses threads.
7. **Walk performance**: call frequency on render paths, payload size and encoding, synchronous calls.
8. **Walk version skew**: how native and script layers are kept compatible, and what happens when they are not.
9. **Route to other lenses**: capability semantics to `input-and-peripherals`; platform runtime to `desktop-native` or `mobile-native`; packaging to `platform-release`.
10. **Stay read-only.**
