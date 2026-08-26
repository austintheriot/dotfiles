---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Desktop-Native Application Development

A reference for reviewing desktop application runtime behavior on Windows, macOS, and Linux: windowing, DPI, OS integration, application lifecycle, filesystem and sandbox, desktop UX conventions, and long-uptime resource behavior. Used by the `desktop-native` subagent.

Distinct from:
- **`mobile-native`**: iOS and Android. We own Windows, macOS, and Linux desktop.
- **`platform-release`**: code signing, notarization, installers, store submission, and update *delivery*. We own the *runtime*.
- **`native-bridge`**: Electron and Tauri IPC internals. We own desktop concerns that apply regardless of shell.
- **`accessibility`**: the a11y catalog. We name the platform accessibility surface and defer depth.
- **`input-and-peripherals`**: pointer, stylus, and device access. We own windowing and OS integration.

**Seams to name explicitly rather than hand off silently:**
- **Launch by file association or URL scheme.** Registration is install-time (`platform-release`); *receiving and routing the activation* is runtime. The bug is almost always on the runtime side.
- **Restart after update.** `RegisterApplicationRestart` is called by the application; `ExitWindowsEx(EWX_RESTARTAPPS)` by the installer. Neither works alone.
- **Hardened runtime entitlements.** Signed at build time, but they gate *runtime* behavior (JIT, library validation, debugger).
- **Sequoia Gatekeeper.** The Control-click override is gone, so distribution friction is now a first-launch runtime experience.

The core thesis: **desktop runtime assumptions do not transfer from mobile or web, and they do not transfer between desktop platforms either.** Lifecycle differs (macOS applications outlive their windows; Windows does not), coordinate spaces differ within a single framework, and the Wayland transition removed capabilities X11 applications took for granted.

Verification markers: **[V]** verified against a primary source, **[U]** unverified.

---

## Framework landscape

### Windows

**Windows App SDK stable is 2.4.0 (2026-08-13) [V]**, with 1.8 leaving maintenance 2026-09-09. Major releases at most every six months; backward compatible to Windows 10 1809.

**WinUI 3 is now unambiguously recommended** [V]: *"the recommended native UI framework for building modern Windows apps... If you're new to Windows development or starting a new project, WinUI is the best place to start."* The years of hedging are over.

**UWP is soft-deprecated**, marked *"Security & bug fixes only"* in Microsoft's own framework table [V]. Existing applications keep working. Note the functional asymmetry: **unpackaged applications get only 4 `ExtendedActivationKind` values against 44 for packaged** [V].

**WPF and WinForms remain actively maintained** [V]; WPF gained a Fluent theme in .NET 9, and both can consume Windows App SDK APIs, since `AppWindow` works with any top-level window handle.

**WinUI still has no drag-and-drop designer** [V] -- a real adoption cost against WPF and WinForms.

### macOS

**Platform numbering jumped to year-based**; there is no macOS 16 through 25. Current shipping is **macOS 26.6.2** [V]; macOS 26 "Tahoe" is the **last Intel-supporting release**, with security updates to fall 2028. **macOS 27 "Golden Gate" has been in public beta since 2026-07-14** [V], and its API surface is already documented -- so a review dated "August 2026" is one cycle behind the leading edge.

AppKit remains the substrate; SwiftUI has closed real gaps but retains a specific remaining list, below.

### Linux

GTK 4.22.0 stable [V], libadwaita 1.9.3, Qt 6.11.2, xdg-desktop-portal 1.22.1, COSMIC epoch-1.7.0. **There is no GTK5 plan** -- mentioned once in GTK 4.9.1 as "still far away" [V].

### Cross-platform

Electron 44.0.0 (8-week majors, **latest three majors supported only**), Tauri 2.11.5, Flutter 3.47.1, Avalonia 12.1.1, Slint 1.17.1, wxWidgets 3.3.3, SDL 3.4.14, winit 0.30.13, Compose Multiplatform 1.12.10-alpha01 (desktop still pre-stable) [V].

**Qt licensing is the genuine trap.** Dual-licensed commercial or LGPLv3 for *some* modules, **with GPL-only modules mixed in -- using one makes the whole application GPL** [V]. LGPL obligations include shipping corresponding library source, notifying users of their rights, and permitting relink with a modified Qt (no tivoization). **Static linking may subject the application itself to LGPL**; dynamic linking is the compliance path. Treat "we will just use LGPL Qt" as a claim requiring a per-module audit. Route the license analysis to `licensing-and-oss`.

---

## SwiftUI on macOS: the specific remaining gaps

All verified against Apple's API metadata [V]. This is the list that decides AppKit-versus-SwiftUI for a given application shape.

**`Table` against `NSTableView`**: `TableColumnCustomization` requires `.customizationID(_:)` on **every** column or the column is silently not customizable; **no cell-reuse contract** (no `makeView(withIdentifier:owner:)`, no public reuse queue); **no row-height API**; **no sections or group rows**; **no `NSOutlineView` equivalent** (`DisclosureTableRow` gives hierarchical rows, not lazy child loading). Live macOS 26 regression: a table not spanning the full content view scrolls content *into* the header, worked around with a 1pt inset.

**Text editing** gained the attributed editor in macOS 26 (`TextEditor` with `AttributedString` binding, `AttributedTextFormattingDefinition`), but: **no multiple cursors** -- Apple, verbatim, *"a single selection value cannot represent multiple cursors"*; **RTF does not round-trip** -- *"this editor does not automatically translate UIKit or AppKit formatting attributes into SwiftUI attributes"*, so import must route through `Transferable`; and no `NSTextLayoutManager` access, typing attributes, marked-text or input-method control, `NSTextViewDelegate` equivalent, or `NSRulerView`.

**Documents**: `DocumentGroup` lacks every autosave control (`autosavesInPlace`, `preservesVersions`, `checkAutosavingSafety()`), the versions browser, `duplicate()`, and all of `NSDocumentController`. **macOS 27 addresses this** with `ReadableDocument` / `WritableDocument`.

**Windows**: SwiftUI gained `WindowLevel`, `UtilityWindow`, `.defaultWindowPlacement(_:)`, and `pushWindow` in macOS 15. **Still missing**: direct `NSWindow` access with no supported bridge (scraping `NSApp.windows` remains the community hack), `NSWindowController`, arbitrary window levels (4 SwiftUI cases against 10 plus arbitrary integers in AppKit), and `restorationClass`.

**Entirely absent from SwiftUI**: printing (`NSPrintOperation` is AppKit-only), file promises (`NSFilePromiseProvider` has no counterpart; `Transferable` does not model a promise), mixed-state menu items, `NSMenuDelegate` for lazy menus, first-responder control, and list virtualization beyond `List`.

**Interop**: `NSHostingSizingOptions` (macOS 13), `NSHostingSceneBridgingOptions` (14), and new in macOS 26, **`NSHostingSceneRepresentation`** for hosting a real SwiftUI `Scene` inside an AppKit lifecycle. Note `dismantleNSView(_:coordinator:)` is **static and cannot capture instance state** -- a common teardown-bug source.

---

## Windows DPI

### The manifest

Namespaces differ **per element**, which is the most common authoring error:

```xml
<asmv3:windowsSettings>
  <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
  <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
</asmv3:windowsSettings>
```

**Precedence** [V]: *"On Windows 10, version 1607, and on, the `<dpiAware>` setting is ignored if the `<dpiAwareness>` element is present."* Older Windows ignores `dpiAwareness`. **Ship both.**

**Per-Monitor V2 is not expressible via `dpiAware` at all** [V]. And a quirk worth flagging: `dpiAware` set to `false` **or any unrecognized string** makes the process unaware **and locks out** `SetProcessDpiAwareness` and `SetProcessDPIAware`.

**Framework support as of 1703** [V]: UWP full; Win32 with comctl32 v6 partial (application-handled); WinForms limited; WPF partial (does not scale hosted cross-framework content); **GDI, GDI+, and MFC: none**.

Per-monitor API substitutions: `GetSystemMetrics` becomes `GetSystemMetricsForDpi`, `AdjustWindowRectEx` becomes `AdjustWindowRectExForDpi`, `SystemParametersInfo` becomes `SystemParametersInfoForDpi`, `GetDpiForMonitor` becomes `GetDpiForWindow`.

**Mixed-mode violations force a process-wide reset** [V]: on Windows 10 1703+, a cross-process `CreateWindow` forces a reset of the caller; in-process `SetParent` across awareness modes **fails** with `ERROR_INVALID_STATE`; cross-process `SetParent` forces a reset of the child's process.

**The virtualization trap, in Microsoft's own words** [V]: *"if a DPI-unaware thread queries the screen size while running on a high-DPI display, Windows will virtualize the answer... this information is not currently sufficiently documented by Microsoft."* Always know the thread's DPI context before calling screen APIs.

### AppWindow and coordinate spaces

`AppWindow` is 1:1 with a top-level window handle and works from WinUI, WPF, WinForms, Win32, and MFC.

**The coordinate-space trap** [V]: XAML `Window.Bounds` uses **effective pixels** while `AppWindow.Position` and `.Size` use **physical device pixels**; converting requires `XamlRoot.RasterizationScale`. This bites custom title-bar drag regions specifically.

Presenter gotchas: a presenter binds to **one window at a time** (reuse throws); `SetPresenter(.Default)` restores the original instance but **not** a custom one, so keep your own reference; `HasTitleBar=true` with `HasBorder=false` throws; `IsModal=true` **without an owner window throws**.

### Snap layouts

**Custom-drawn maximize buttons break snap layouts** [V]. The fix is returning `HTMAXBUTTON` from `WM_NCHITTEST`, noting the message carries **screen** coordinates that must be converted with `MapWindowPoints`.

**A second, less-known failure**: the menu appears but snapping does not work when the minimum window size is too large. Microsoft's guidance is a minimum width of *at most* 500 effective pixels, recommended 330 or less [V].

**DWM attribute version gates differ** [V]: dark mode (20) and corner preference (33) need build 22000, but **`DWMWA_SYSTEMBACKDROP_TYPE` (38) needs 22621** -- gate it separately. And *"all windows default to light mode regardless of the system setting"*, so dark mode is opt-in per window and **affects the frame only**. There is **no documented Win32 client-area dark-mode API** [V].

---

## macOS windowing

**`CollectionBehavior` mutual exclusivity**, verbatim [V]: *"`primary`, `auxiliary`, and `canJoinAllApplications` only apply to full screen and Stage Manager. They're also mutually exclusive. Specify at most one per window."*

Window levels are ordered absolutely: *"Even the bottom window in a level will obscure the top window of the next level down."*

**State restoration**: `restorationClass`, `isRestorable`, `encodeRestorableState(with:)`. **`applicationSupportsSecureRestorableState(_:)` is macOS 12.0** and is **absent from the delegate's own topic sections in Apple's documentation** [V] -- a discoverability trap that explains how widespread the console warning became.

---

## The Wayland transition

### Status

**GNOME 50 (March 2026) removed the X11 session option entirely** [V]; mutter's build options now expose only `xwayland`. The remaining `src/x11` tree is **XWayland, not the X11 session** -- do not confuse them. **KDE Plasma still ships an X11 backend** [V], which is the sharpest desktop-environment divergence in 2026.

### What breaks, with mechanism

**Global hotkeys.** No `XGrabKey`. The replacement is the `org.freedesktop.portal.GlobalShortcuts` D-Bus portal. **GNOME implements it as of GNOME 48 (January 2025)** [V]; **the 2026 gap is wlroots/Sway and COSMIC**, which do not.

**XWayland `XGrabKey` is an allowlist, not a focus-only fallback** [V]. Mutter's default allowlist is VM and remote-desktop viewers matched on `res_class`; **a normal application is refused** and receives keys only when focused.

**Window positioning is gone.** GTK4 **removed the APIs at compile time** [V] -- `gtk_window_move()`, `set_position()`, `set_keep_above()`, and *"any APIs that deal with global (or root) coordinates"*. Migration guidance, verbatim: *"Most likely, you should just stop using them."* Qt compiles and **silently no-ops**, returning artificial values such as `QPoint(0, 0)`. **SDL3 did not remove positioning** -- the calls exist and **fail at runtime** on Wayland toplevels.

**Position restore does not work in 2026** [V]. `xdg_positioner` is popups only; `xdg_toplevel` has no positioning request; session restore lives in experimental `xx-session-management-v1` with **no compositor support**. **Restore size; let the compositor place.**

**Screen capture** goes through the ScreenCast portal plus PipeWire. The picker is not necessarily shown every time -- `persist_mode` returns a `restore_token`, but **tokens are single-use**: each session returns a fresh one you must store, or you re-prompt every launch [V].

**Clipboard** reads require focus and a valid input serial, so unfocused reads and clipboard-manager polling are impossible. **`ext-data-control-v1` is the sanctioned answer**; **`wlr-data-control-unstable-v1` is now explicitly deprecated** [V].

**Always-on-top is compositor policy** [V]: Electron documents that `setAlwaysOnTop` *"has no effect on Wayland"* and that `isAlwaysOnTop` **returns stored state, not reality**.

**Fractional scaling** uses `wp_fractional_scale_v1` plus `wp_viewporter`: keep buffer scale at **1**, render `surface_size x scale`, set the viewport destination to the logical size. **Toolkit support is far earlier than commonly assumed -- GTK 4.12 (2023) and Qt 6.5 LTS** [V]; compositor support is effectively universal. `doc.qt.io/highdpi.html` is stale and still claims integer-only -- do not cite it.

**XWayland has no fractional-scale path**, so scaled X11 clients render at integer scale and get downscaled. This is the main visible penalty for staying on XWayland.

**Java has no Wayland toolkit in any JDK** [V]. JDK-8281970 remains open with no activity since 2022, so Java and JavaFX desktop applications are entirely XWayland-dependent.

**Decorations**: ship client-side; GNOME is CSD-only in practice, and server-side decoration is a bonus.

**Electron** defaulted to Wayland in 38.0.0 (2025-09-02) [V], with 38.2.0 the honest "works" version. Live bugs worth knowing: `globalShortcut.register()` returns `true` before the async portal bind resolves, and portal command IDs derive from an accelerator-to-string conversion that **returns empty for all function keys, Escape, Enter, most punctuation, and numpad**, collapsing every such accelerator to one ID so only the last survives.

---

## Application lifecycle

### Windows shutdown: five seconds, twice

`WM_QUERYENDSESSION` `lParam` carries **bit flags** -- test bitwise, never by equality. `lParam == 0` means shutdown or restart, indistinguishable.

Verbatim [V]: applications *"can delay responding to `WM_QUERYENDSESSION` for 5 seconds, then the system allows the user to continue or cancel shutdown. Applications that return **TRUE**... can delay responding to `WM_ENDSESSION` for 5 seconds."*

**Console and windowless applications cannot cancel shutdown at all** [V].

**The protocol**: return immediately from `WM_QUERYENDSESSION`; defer all cleanup to `WM_ENDSESSION`. Block only via `ShutdownBlockReasonCreate`. Microsoft: *"Applications cannot rely on being able to block shutdown."*

### Single instance

**Named mutex**: name is **case sensitive**, `Global\` spans terminal-server sessions while `Local\` is per session, and **the remainder cannot contain a backslash**. The namespace is shared with events, semaphores, and file mappings, so a collision returns `ERROR_INVALID_HANDLE`.

**Microsoft's security warning, verbatim** [V]: *"If you are using a named mutex to limit your application to a single instance, a malicious user can create this mutex before you do and prevent your application from starting. To prevent this situation, create a randomly named mutex and store the name so that it can only be obtained by an authorized user."*

**Windows App SDK `AppInstance` differs from UWP in that redirection is not terminal** [V] -- UWP terminated after redirect, so you must guard against circular A-to-B-to-C-to-A chains. And an **STA deadlock trap** that bites WPF and WinForms: *"the calling app must wait for the method to complete, otherwise the redirection will fail. However, waiting on an async call will block the STA."* The documented workaround is another thread plus a semaphore.

**macOS**: `applicationShouldTerminateAfterLastWindowClosed(_:)` **defaults to `false`** [V] -- the application stays running with no windows, which is the idiom. `applicationShouldTerminate(_:)` supports deferred termination via `.terminateLater`.

**`application(_:open urls:)` suppresses `application(_:openFile:)` entirely when implemented** [V], and the URL array **excludes URLs matching your declared document types**.

### Power

**Windows `SetThreadExecutionState`**: **without `ES_CONTINUOUS` the call resets the idle timer once**, so it must be called periodically. Away mode does not affect the sleep idle timer. Two hard limits [V]: it *"cannot be used to prevent the user from putting the computer to sleep"* and *"does not stop the screen saver from executing."*

**Windows QoS levels** are the responsible-background-work lever [V]: **In Focus is High, Visible is Medium, Minimized or Fully Occluded is Low.** EcoQoS is explicit opt-in via `SetProcessInformation`; setting both masks to 0 resets to system-managed.

**Fast Startup means a user-initiated "shutdown" is actually hibernation with the user logged off** [V], so kernel and driver uptime spans reboots -- Microsoft warns about memory leaks accordingly, and wake-on-LAN does not work from it.

**macOS App Nap**: prefer `beginActivity(options:reason:)` over raw power assertions.

### Documents and crash recovery

**Apple's `write(to:ofType:)` warning is the single most common `NSDocument` bug** [V]. Overrides must assume nothing about the destination -- *"This location might be a hidden temporary directory"* -- nor the filename, which *"may have no obvious relation to the document name"*. The indirection preserves creation date, permissions, Finder icon position, and user aliases.

**Windows Restart Manager**: `RegisterApplicationRestart` **must not include the executable name** (it is prepended), and *"the system will only restart the application if it has been running for a minimum of 60 seconds"* [V]. Note the asymmetry: crash and hang prompt the user, while **update-driven restart is automatic**.

---

## OS integration

### Notifications

**The legacy AUMID and COM-activator apparatus is obsolete for unpackaged Windows applications** [V]. Microsoft, verbatim: *"For unpackaged apps, `Register()` automatically sets up the COM server registration... You don't need to configure COM activation or an AUMID manually."*

**Required call order** (stated as Important twice) [V]: register the `NotificationInvoked` handler, then `AppNotificationManager.Default.Register()`, **then** `GetActivatedEventArgs()`.

Three gotchas: **elevated applications cannot send or receive notifications -- `Show` fails silently** [V]; `NotificationInvoked` fires on a **background thread**; and `activationType="background"` **is ignored for desktop applications**.

AUMID rules still matter for taskbar grouping: max 128 characters, no spaces, set before any UI. A window-level AUMID **overrides** the process-level one, after which the file dialog can no longer infer it.

### Tray and status items

**`NIM_SETVERSION` must follow every `NIM_ADD`** [V] -- *"The version setting is not persisted once a user logs off."*

**`NOTIFYICON_VERSION_4` repacks the callback**, which is the part most often wrong: the event is in `LOWORD(lParam)`, the icon ID in `HIWORD(lParam)` (**16 bits only**), and anchor coordinates in `wParam` -- **undefined for messages other than** `NIN_POPUPOPEN`, `NIN_SELECT`, `NIN_KEYSELECT`, and the mouse range [V].

**GUID identification is bound to the binary's file path** [V]; moving the executable breaks the icon and its settings.

**Windows 10 and 11 diverge** [V]: on Windows 10 balloon messages persist in the Notification Center; **on Windows 11 they are transient and do not persist**.

### Clipboard and drag-drop

**`EmptyClipboard` assigns ownership to you** and sends `WM_DESTROYCLIPBOARD` to the previous owner. Set formats **most descriptive first**; readers take the first recognized.

**Ownership transfers to the system** [V], which frees standard formats -- but **private formats are never freed by the system** and must be released on `WM_DESTROYCLIPBOARD`.

**Delayed rendering has two opposite rules** [V]: in `WM_RENDERFORMAT` you *"must not open the clipboard"* because the requester holds it; in `WM_RENDERALLFORMATS` you **must** open it, verify ownership, and set all formats or the data ceases to exist.

Microsoft now quantifies the tradeoff: delayed-rendering overhead is roughly 10 to 100 microseconds, which *"consistently exceeded the cost to copy the data... if the data was 100 KiB or less"* [V]. Place directly for single-format text under 4 KiB.

**Elevated windows need `ChangeWindowMessageFilterEx`** or UIPI silently drops drag-drop messages.

**macOS pasteboard privacy is new and disruptive** [V]: `NSPasteboard.AccessBehavior` (macOS 15.4+) defaults to **asking the user on programmatic General-pasteboard access**. **`changeCount` polling for clipboard monitoring is squarely in the crosshairs**; use `detectedPatterns(for:)` for non-alerting inspection.

### File associations and defaults

**`HKEY_CLASSES_ROOT` is a merged view** -- write to a real hive, never HKCR. Call `SHChangeNotify(SHCNE_ASSOCCHANGED, ...)` after changes *"otherwise the change may not be recognized until after the system is rebooted"* [V].

**Counterintuitive uninstall rule** [V]: remove your ProgIDs but **do not clear the `.ext` Default** pointing at them -- Windows ignores a Default naming an unregistered ProgID, and clearing it risks stomping a later application's ownership.

**Defaults are user-controlled and programmatically unsettable** [V]. Verbatim: *"As of Windows 8, the only functionality of this interface that is supported is `QueryCurrentDefault`."* You can read the default and register candidacy; you cannot become it. The `UserChoice` hash is the enforcement.

### Startup

**Windows Run key**: value data is capped at 260 characters, **ordering among entries is explicitly indeterminate**, and *"the system may choose to delay the execution of programs in the `Run` key"* [V].

**Packaged `StartupTask`**: packaged desktop applications get no consent dialog, but **the user always wins** -- once `DisabledByUser`, *"`RequestEnableAsync` will not override their choice"* [V].

**macOS `SMAppService`** (macOS 13+) replaced the deprecated `SMLoginItemSetEnabled`.

---

## Filesystem and sandbox

### macOS sandbox and hardened runtime

Apple's rules [V]: *"You add entitlements **only to executables**. Shared libraries, frameworks, and in-process plug-ins **inherit** the entitlements of their host executable."* macOS **refuses to load system extensions** using hardened-runtime exception entitlements. *"Don't include an entitlement if the value is false."* Notarization requires hardened runtime.

**A documentation finding worth acting on** [V]: `com.apple.security.files.bookmarks.app-scope`, `.document-scope`, and the entire `temporary-exception.files.*` family are **absent from Apple's current entitlements index**. The strings still function, but the de-listing is a meaningful App Store review signal -- temporary exceptions require written justification and are routinely rejected.

**Security-scoped bookmarks**, verbatim [V]: *"You must balance each call to `startAccessingSecurityScopedResource()`... with a call to `stopAccessingSecurityScopedResource()`."*

**The leak is kernel resources, not file descriptors** [V]: *"If you fail to relinquish your access... your app leaks kernel resources. If sufficient kernel resources leak, your app loses its ability to add file-system locations to its sandbox... until relaunched."*

**Persisting a plain path instead of a bookmark** means the path carries no scope, the sandbox has no record on relaunch, access fails with `EPERM`, and the only recovery is re-prompting.

### macOS TCC

**Prompt on first use** with a usage string: camera, microphone, location, contacts, calendars, photos, Apple Events, other applications' containers, local network.

**Manual only -- the user must open System Settings** [V]:
- **Full Disk Access has no preflight API at all.** The convention is attempting a known-protected read and inferring from `EPERM`.
- **Accessibility**: `AXIsProcessTrusted()`, and **the prompt only offers to open Settings -- it cannot grant.**
- **Screen Recording**: prompts once, then manual.

**Sequoia Gatekeeper**, verbatim [V]: *"In macOS Sequoia, users will no longer be able to Control-click to override Gatekeeper."*

### Windows filesystem

**Reserved names are reserved with any extension** [V] -- `NUL.txt` and `NUL.tar.gz` are both `NUL`. Windows treats **superscripts as digits** in device names. Do not end a name with a space or period.

**Long paths need registry *and* manifest** [V], and the registry value *"will be cached by the system (per process) after the first call to an affected Win32 file or directory function"* and is not reloaded for the process lifetime.

**`\\?\` disables normalization** -- no forward slashes, no `.` or `..`. **Relative paths can never use it, so they are always capped at MAX_PATH.** Shell APIs are not in the long-path opt-in list: *"It is possible to create a path with the Windows API that the shell user interface is not able to interpret properly"* [V].

**Atomic replace uses `ReplaceFileW`**, preserving creation time, DACLs, object identifier, and named streams from the original, requiring **all three files on the same volume**. `REPLACEFILE_WRITE_THROUGH` is documented **"not supported"** [V]. Its three partial-failure error codes mean materially different states and must be handled distinctly -- in one, **the replaced file no longer exists**.

**`ReadDirectoryChangesW` -- design around the overflow** [V]. Verbatim: *"If the buffer overflows, `ReadDirectoryChangesW` will still return **true**, but **the entire contents of the buffer are discarded** and the `lpBytesReturned` parameter will be zero."* **A successful return with zero bytes means you lost every event and must re-enumerate.** Any watcher without a full-rescan fallback is incorrect under load.

**MSIX redirection is narrower than assumed** [V], gated on `TrustLevel: appContainer`. **Writes inside the package are not allowed** (the package is read-only and tamper-checked) rather than silently redirected. Packaging's baseline purpose is **package identity, not sandboxing**.

**UAC**: *"Specifying **requestedExecutionLevel** node will **disable file and registry virtualization**"* [V]. Under UAC, per-machine post-install default changes fail, so the pattern is **per-machine install, per-user data and defaults**.

### Linux

**`XDG_RUNTIME_DIR` has no default** and must be user-owned at mode 0700, created at login and removed at logout [V].

**Flatpak blocks by default**: all host files outside the app's own directories, network, device nodes, and session bus beyond its own namespace. Note `--socket=pulseaudio` **includes microphone input**, and `--socket=session-bus` is documented as a *"security risk, generally avoided"*. Portals are the sanctioned path, where *"the user's selection of a file... is interpreted as implicitly granting the application access"* [V].

---

## Desktop UX conventions

**Divergences a review should catch**: **Redo** is Shift-Ctrl-Z on GNOME and macOS but **Ctrl-Y on Windows**; **Preferences** is Cmd-comma on macOS and Ctrl-comma on GNOME but lives under Tools then Options on Windows; **Help** is F1 on Windows and GNOME but a menu on macOS; **macOS has no mnemonics**, only key equivalents, while Windows and Linux distinguish Alt-reachable mnemonics from direct accelerators; and **Windows has no application-level quit shortcut** (Alt-F4 closes a window).

**Text editing is the most common cross-platform toolkit tell**: on macOS, Home and End go to document start and end while line ends are Cmd-arrow, word-jump is Option-arrow, and **emacs bindings work in every text field**. Windows and Linux scope Home and End to the line and use Ctrl-arrow for word-jump.

**Document model**: macOS gives autosave-in-place, Versions, and **Duplicate instead of Save As** free via `NSDocument`. Windows and Linux have no framework-level equivalent and must build it.

**Printing** is first-class on macOS with PDF export built into the print panel; **SwiftUI still has no printing API**.

**Theming**: macOS `NSAppearance`; Windows frame-only opt-in dark mode with **no documented client-area API**; Linux via the Settings portal `color-scheme`. Reduced motion and high contrast exist on all three and are commonly ignored.

---

## Long-uptime resources

**Windows GDI: the folklore numbers are wrong** [V]. There is *"a theoretical limit of 65,536 GDI handles per session"* -- **per session, and theoretical**. The quota is settable from 256 to 65,536. **The commonly cited 10,000 per-process default is not stated in Microsoft's documentation.**

The **creator/destroyer asymmetry** is the classic leak: a device context from `GetDC` needs `ReleaseDC`, one from `CreateDC` needs `DeleteDC`, and objects need `DeleteObject`.

**Timer resolution: two separate changes, and the first is not Windows 11** [V]. Per-process resolution arrived in **Windows 10 version 2004**; Windows 11 *separately* revokes higher resolution when a window-owning process becomes fully occluded or minimized. Higher resolution *"can prevent the CPU power management system from entering power-saving modes."* Match every `timeBeginPeriod` with `timeEndPeriod`.

**Memory priority** is the documented lever for background work, since low-priority pages trim first.

---

## Anti-pattern catalog

### Windows DPI and windowing
- No DPI-awareness manifest, so the system bitmap-stretches the application.
- `dpiAware` only with no `dpiAwareness`; Per-Monitor V2 is not expressible that way.
- `dpiAware` set to `false` or a typo, which also **locks out** the runtime awareness APIs.
- Ignoring the suggested rectangle in `WM_DPICHANGED`, causing cursor drift and recursive DPI-change loops.
- Caching fonts and metrics at initialization rather than re-evaluating per `WM_DPICHANGED`.
- `GetSystemMetrics` on a per-monitor-aware thread instead of the `ForDpi` variant.
- `SetParent` across awareness modes, failing or forcing a process-wide reset.
- Custom-drawn maximize button without `HTMAXBUTTON`, silently killing snap layouts.
- Minimum window width above 500 effective pixels, so snapping fails though the menu appears.
- Mixing XAML effective pixels with `AppWindow` device pixels in drag-region math.

### Windows lifecycle and integration
- Cleanup inside `WM_QUERYENDSESSION` rather than `WM_ENDSESSION`.
- A windowless or console application returning FALSE expecting to cancel shutdown.
- A predictable named mutex for single-instance, which a local attacker pre-creates to block startup.
- `Global\` versus `Local\` chosen without considering fast user switching.
- Blocking on redirect activation from an STA, deadlocking WPF and WinForms.
- Redirect chains with no cycle detection.
- An elevated application sending toasts, which **fail silently**.
- Tray icon not re-added on Explorer restart.
- `NIM_ADD` without `NIM_SETVERSION`, or reading `wParam` coordinates for non-mouse tray messages.
- Moving an executable whose tray icon is GUID-registered by path.
- Drag-drop into an elevated window without the message filter.
- Opening the clipboard inside `WM_RENDERFORMAT`, or failing to open it in `WM_RENDERALLFORMATS`.
- Attempting to set the default file handler programmatically.
- Clearing the `.ext` Default on uninstall.
- A filesystem watcher with no re-enumeration path.
- `MoveFileEx` where `ReplaceFile` was needed.
- Long-path manifest without the registry value, or vice versa.
- Writing application data to the install directory.

### macOS
- Persisting a file path instead of a security-scoped bookmark.
- Unbalanced `startAccessingSecurityScopedResource()`, leaking **kernel resources**.
- Assuming last-window-close quits the application.
- Implementing `application(_:open:)` and still expecting `openFile:` to fire.
- `NSDocument.write(to:ofType:)` assuming the destination path or filename.
- Expecting an API to prompt for Full Disk Access, or treating the Accessibility prompt as a grant.
- `changeCount` polling for clipboard monitoring under macOS 15.4+ privacy behavior.
- Missing `applicationSupportsSecureRestorableState(_:)`.
- Expecting RTF to round-trip through the macOS 26 attributed text editor.

### Linux and Wayland
- Global hotkeys via `XGrabKey`, refused under XWayland's allowlist.
- Global hotkeys assumed working after adding the portal, with no support on wlroots/Sway or COSMIC.
- Absolute window positioning, removed at compile time in GTK4 and silently no-op in Qt.
- "Restore window to last position", which has no compositor support.
- `setAlwaysOnTop` on Wayland, where the getter also lies.
- Reading the clipboard while unfocused.
- Screen capture without storing the fresh single-use restore token, re-prompting every session.
- Shipping server-side-decoration-only, which GNOME does not provide.
- Rendering at integer scale under fractional scaling.
- Shipping a Java or JavaFX desktop application expecting native Wayland.

---

## Schools of thought (preserve disagreement)

- **Native per platform versus one cross-platform toolkit.** Note the asymmetry this reference surfaces: **the runtime concerns here do not disappear with a cross-platform toolkit; they get abstracted, sometimes incorrectly.** Electron's Wayland global-shortcut bugs are the evidence.
- **Electron resource cost versus shipping velocity.** Tauri's system-webview approach trades binary size for **webview fragmentation across WKWebView, WebView2, and WebKitGTK**, which is a different bug surface, not a strictly smaller one.
- **Qt licensing.** Genuinely unsettled and module-dependent. The Qt Company has an obvious interest, which is itself a reason to read the obligations rather than the FAQ.
- **SwiftUI on macOS readiness.** The verified gap list supports AppKit **for document-, table-, and text-heavy applications** and SwiftUI for most others. macOS 27 moves the line again.
- **Platform conventions versus one consistent experience.** Text-editing bindings and menu placement are where users notice immediately.
- **Sandbox versus Developer ID.** Temporary exceptions being de-listed from Apple's index is a directional signal; Sequoia's Gatekeeper change raised Developer ID first-launch friction.
- **Wayland's security model versus application capability.** Both positions are true: compositor-mediated consent is correct, *and* the portal surface is incomplete and fragmented today.
- **Packaged versus unpackaged on Windows.** The `ExtendedActivationKind` asymmetry of 4 against 44 is an underappreciated functional difference.

---

## What is NOT a desktop-native finding

- iOS or Android behavior. Route to `mobile-native`.
- Signing, notarization, installers, store submission, update delivery. Route to `platform-release`.
- Electron or Tauri IPC internals and bridge contracts. Route to `native-bridge`.
- The accessibility catalog beyond naming the platform surface. Route to `accessibility`.
- Pointer, stylus, and peripheral access. Route to `input-and-peripherals`.
- Qt or GPL license analysis. Route to `licensing-and-oss`.
- Generic "use WinUI" or "rewrite in Tauri" advocacy without a named defect.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: data loss or unrecoverable state -- a filesystem watcher with no rescan path (silent event loss), unbalanced security-scoped access (application loses sandbox capability until relaunch), cleanup in `WM_QUERYENDSESSION` (killed mid-write at 5 seconds), `MoveFileEx` where `ReplaceFile` was required, a predictable single-instance mutex.
- **major**: missing or wrong DPI manifest; ignoring `WM_DPICHANGED` geometry; elevated toasts failing silently; tray icon lost on Explorer restart; `NIM_SETVERSION` omitted; persisting paths instead of bookmarks; assuming last-window-close quits on macOS; global hotkeys assumed working on Wayland; absolute positioning under Wayland; clipboard monitoring by `changeCount` polling under current macOS.
- **minor**: `DWMWA_SYSTEMBACKDROP_TYPE` not version-gated separately; missing `applicationSupportsSecureRestorableState`; server-side-decoration-only on Linux; integer-scale rendering under fractional scaling; non-conventional shortcut for Redo or Preferences.
- **nit**: menu ordering details; window title conventions.
- **insight**: structural -- "this application is document-, table-, and text-heavy, which is precisely the shape where SwiftUI's remaining macOS gaps bite; the AppKit interop cost is likely lower than the workaround cost"; "positioning logic assumes X11 and will silently no-op for every GNOME user"; "no rescan path exists, so this watcher is correct only under light load."

Confidence: high when the trigger is a concrete API call, manifest value, or message handler; medium when reasoned from application shape. **Note platform-version gates before flagging**, and date any claim about a shipping release.

---

## Process for the desktop-native agent

1. **Identify the platforms and the framework.** Win32, WinUI, WPF, WinForms, AppKit, SwiftUI, GTK, Qt, or a cross-platform shell. The catalog differs sharply.
2. **Identify the packaging and sandbox posture**: packaged or unpackaged on Windows, sandboxed or Developer ID on macOS, Flatpak or native on Linux. This gates most filesystem findings.
3. **Walk DPI and coordinate spaces** on Windows: manifest, awareness mode, per-monitor API use, mixed coordinate spaces in window math.
4. **Walk windowing**: level and collection behavior, multi-monitor, state restoration, and on Linux whether any positioning or z-order assumption survives Wayland.
5. **Walk lifecycle**: launch paths including file and URL activation, single-instance mechanism, shutdown handling and its time budget, quit semantics against the platform idiom.
6. **Walk OS integration**: notifications, tray, clipboard and drag-drop, file associations, startup registration.
7. **Walk the filesystem**: correct per-platform data locations, reserved names, atomic replace, watchers and their overflow path, sandbox access persistence.
8. **Walk power and long-uptime behavior**: sleep prevention limits, background QoS, handle and object lifetime.
9. **Check UX conventions** against the target platform, particularly text editing and shortcuts.
10. **Route to other lenses**: signing and delivery to `platform-release`; bridge internals to `native-bridge`; a11y depth to `accessibility`; input devices to `input-and-peripherals`; licensing to `licensing-and-oss`.
11. **Stay read-only.**
