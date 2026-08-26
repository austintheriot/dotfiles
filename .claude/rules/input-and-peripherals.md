---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Input and Peripherals

A reference for reviewing input handling and device-capability access: pointer, touch, stylus, keyboard, gamepad, and text input, plus the permission models and platform APIs for cameras, microphones, Bluetooth, location, files, and other hardware. Used by the `input-and-peripherals` subagent.

Distinct from:
- **`browser-spec`**: the DOM event model and platform-primitive reinvention. We own input fidelity and device access; they own whether a platform API already does the job.
- **`native-bridge`**: how a request crosses a bridge to reach a capability. We own what the capability does and what its permission model requires.
- **`accessibility`**: assistive technology and semantics. We own raw input mechanics; alternative input paths route there.
- **`mobile-native`** / **`desktop-native`**: platform runtime broadly. We own the input and device surface specifically.
- **`graphics-programming`**: rendering the stroke. We own capturing it.

The core thesis: **input fidelity is lost silently, and permissions fail in ways that do not look like permission failures.** Dropped input samples produce a stroke that feels wrong with no error anywhere. A missing purpose string crashes rather than denies. A granted permission can return a filtered subset. None of these surface as the problem they are.

The operational priority: **check what the permission actually returns, not whether it was granted.** The modern failure shape is a successful-looking grant paired with silently reduced data.

Verification markers: **[V]** verified against primary source, **[U]** unverified.

---

## Universal principles

### Missing purpose strings crash; they do not deny

On Apple platforms, **absence of a usage-description string is not a denial -- it is a hard termination** [V]. The permission system needs a string to render its alert; with none there is nothing to display, so the system raises an exception and the process aborts at first access.

**It cannot be defended against by checking authorization status first**, and it is **invisible on a device that was previously authorized**, so it commonly ships after passing local testing.

### A granted permission can return less than it says

**Android 14's visual-media false grant is the sharpest example** [V]. Requesting `READ_MEDIA_IMAGES` or `READ_MEDIA_VIDEO` without also declaring `READ_MEDIA_VISUAL_USER_SELECTED` returns **`PERMISSION_GRANTED`** while the media store silently returns only the user's selection, and verbatim: **"This false grant state will persist until the app goes into the background."**

The permission check says yes, the query returns a subset, and the state resets on backgrounding. Code that branches on the grant and assumes full access is wrong in a way no error surfaces.

### Prefer the out-of-process picker over the permission

**This is the universal escape hatch** and it should be the default recommendation. `PHPickerViewController`, the Android Photo Picker, macOS Powerbox, and the Windows file picker all run in a **separate process**, render the user's library without granting the application any library-wide capability, and vend only what the user tapped.

`PHPickerViewController` lives in `PhotosUI` rather than PhotoKit, and **needs no permission and no usage-description string at all** [V].

If the feature is "the user picks specific items," you can usually drop the permission entirely -- and given store-review scrutiny and platform policy, you should.

### Silent-eviction caps are the dominant modern failure shape

Persisted access grants have limits that **fail without exception** [V]: Android's storage-access framework caps persisted URI grants at **512 with silent oldest-first eviction**, and `takePersistableUriPermission()` throws nothing on overflow. Photo-picker grants and platform recent-file lists have analogous caps.

The odd one out is Windows's future-access list, which **hard-fails rather than evicting** -- and pairing it with a self-evicting recent list is precisely what hides the failure.

### Android breaking changes land on target-SDK bumps

Behavior changes gate on `targetSdkVersion`, not on the operating system version [V]. **The blast radius is every device at once**, decoupled from any user-visible event, which is why these regressions appear in a release with no obvious trigger.

---

## Pointer, stylus, and touch

### Input fidelity: coalesced and predicted events

The single most consequential input concept for drawing surfaces.

A touchscreen samples faster than the display refreshes -- often 120 Hz to 240 Hz against 60 Hz or 120 Hz of frames. **The system delivers one event per frame and buffers the rest.** Reading only the delivered event **discards most of the samples**, producing visibly polygonal strokes at speed.

- **Web**: `getCoalescedEvents()` returns the buffered samples for the frame; `getPredictedEvents()` returns extrapolated future points to mask latency. Pointer Events Level 3 reached **W3C Recommendation on 2026-06-30** [V].
- **Apple**: `coalescedTouches` and `predictedTouches` on the event.
- **Android**: batched historical samples on the motion event, plus a motion-prediction library.

**Flag**: a drawing or gesture path that reads only the primary event; prediction rendered into the committed stroke rather than a transient layer, which leaves visible artifacts when the prediction is wrong.

### Stylus properties

Pressure, tilt, and orientation are the properties that make a stroke feel like a pen. On the web these are `pressure`, `tiltX`, `tiltY`, `twist`, and `pointerType`; on Apple, force with altitude and azimuth angles; on Android, pressure, tilt, and orientation axes with distinct tool types for stylus and eraser.

**Flag**: treating pressure as binary; assuming pressure is available (many devices report a constant); ignoring the eraser tool type so the eraser draws; hardcoding a tilt range.

**Palm rejection** is partly platform-provided and partly the application's job. Where the platform does not classify, the application must, and the naive approach of ignoring large touches also rejects legitimate broad contact.

**Apple Pencil Pro APIs** (squeeze, barrel roll, haptics) **could not be verified** [U] because the relevant documentation resists automated retrieval. Verify symbol names against Apple's documentation before relying on them.

### Hover, precise location, and hit testing

Hover exists on some stylus and pointer hardware and should enhance rather than gate. Prefer the precise-location accessor where the platform offers one, since the default may be quantized.

---

## Capability access

### Camera, photos, and microphone

Prefer the picker. Where the camera itself is required, the modern APIs are the platform capture frameworks, and on the web `getUserMedia`.

**iOS WebView note** [V]: `getUserMedia` in an embedded web view is gated by the **host application's** usage descriptions, and if the delegate decision hook is not implemented, the system defaults to prompting.

**Audio session behavior is where most audio bugs live.** Two specific obligations:

- **Unplugging headphones is not an interruption** [V]. No interruption notification fires; the session silently reroutes to the speaker and keeps playing. Apple's framing is direct: miss the old-device-unavailable reason and **unplugging headphones broadcasts the audio to the room.** High-level players handle this; raw engine code does not.
- **On Android, permanent audio-focus loss never returns** [V]. Treating it as transient leaves a player waiting forever for a gain callback that cannot arrive. Distinguish permanent from transient loss.

Note also that a category constant was renamed with **no compiler warning**, and that Android 17 fails background audio capture **silently** -- no exception, no callback, just silence in the recording.

### Bluetooth

**Android 12 split the permissions** into scan, connect, and advertise. The historical location requirement was not bureaucratic: fixed-position beacons with known coordinates make an unprivileged scan a geolocation oracle. **Declaring `neverForLocation` avoids the location prompt but has a documented cost** -- some beacons are filtered from results [V].

**The scan throttle is real and source-confirmed**: roughly **5 scans per 30-second window**, though it is runtime-tunable so the number is not guaranteed across devices [V].

**Android 14 changed MTU semantics** [V]: the stack requests a 517-byte MTU when the first client asks and **disregards all subsequent requests**. Code that negotiates a specific value and trusts the result is silently wrong.

**Web Bluetooth's reach is narrower than assumed** [V]: advertisement watching is officially not being pursued, and the reconnect-without-prompt methods remain flag-gated. **Web views report no support at all, so Cordova and Capacitor wrappers cannot use it.** Mozilla and WebKit hold written positions against it, so this is policy rather than lag.

In Electron, the device-selection handler **auto-selects the first device** unless the default is prevented [V] -- the most common Electron Bluetooth bug.

The specification blocklist is worth understanding: human-interface services are excluded because direct access would let a page become a keylogger, and firmware-update services because unsigned update is device takeover.

### Files

**Security-scoped bookmarks are macOS-only**; iOS uses an implicit ephemeral scope valid until reboot at the latest [V]. Apple's warning on failing to balance access is unusually blunt: the application **leaks kernel resources and loses its ability to add any filesystem location to its sandbox until relaunched.**

Two legacy entitlement strings for bookmarks **appear in no current documentation and return not-found everywhere** [V]. Do not add them.

**File System Access on the web**: pickers landed in Chrome 86 and **are supported on Chrome Android** [V] -- a correction to widely repeated claims otherwise. Safari and Firefox hold written positions against. The origin-private filesystem is genuinely cross-browser.

**The permission consequence to design around**: the handle survives in storage but **the grant does not**, and re-requesting permission **requires transient user activation**. You cannot silently reopen the last document; you must render a button the user presses.

### Location

**Provisional always-authorization is a deferred grant, not a lesser one** [V]: the platform uses two prompts, and the second typically appears **when the application is not running**. It cannot be detected and worked around at request time.

**Android has two quiet failures** [V]: requesting fine location alone is ignored on some releases with only a log message, and **requesting foreground and background together means the system ignores the request and grants neither** -- which presents as "location is broken," not "background was denied." Android 11 and later removed the always-allow option from the dialog entirely.

**On the web, the geolocation timeout defaults to infinity** [V], so an unanswered prompt hangs the callback forever.

Prefer coarse where it suffices; precise location is a sensitive category under multiple privacy regimes and draws review scrutiny.

### Other devices

USB, human-interface devices, serial, and MIDI on the web are **Chromium-only by written vendor opposition** [V], and unavailable in web views, so wrapper applications cannot reach them. Native paths exist per platform, and Apple's external-accessory route requires program membership.

Biometrics gate a local authentication result, not a credential; the secure enclave or keystore holds the key material, and access-control flags determine whether a biometric change invalidates it.

Notifications require runtime permission on Android 13 and later. Background execution is best-effort on every platform.

---

## Anti-pattern catalog

### Input fidelity
- Reading only the primary pointer event on a drawing surface, discarding buffered samples.
- Rendering predicted points into the committed stroke rather than a transient layer.
- Treating pressure as binary, or assuming it varies at all.
- Ignoring the eraser tool type so the eraser draws.
- Hardcoding tilt or pressure ranges.
- Palm rejection by touch size alone, which also rejects legitimate contact.
- Gating a feature on hover, which much hardware lacks.
- Using quantized coordinates where a precise accessor exists.

### Permissions
- Requesting every permission at launch rather than at point of use; on Apple platforms a denial **cannot be re-prompted** and the user must visit settings.
- Missing usage-description string, which **crashes rather than denies** and is invisible on a previously authorized device.
- Branching on a granted media permission without declaring the user-selected variant, so a full-access path runs against a filtered subset.
- Requesting foreground and background location together on Android, so **neither is granted**.
- Requesting fine location alone on affected Android releases.
- Requesting a library permission where an out-of-process picker would need none.
- Scanning for Bluetooth without `neverForLocation`, forcing a location prompt users refuse -- or declaring it without accounting for filtered beacons.

### Device behavior
- Assuming a negotiated Bluetooth MTU is honored on Android 14 and later.
- Scanning more often than the throttle allows and treating silence as absence of devices.
- Not handling the old-device-unavailable route change, so unplugging headphones plays audio aloud.
- Treating permanent audio-focus loss as transient, waiting forever for a gain callback.
- Persisting file access beyond the eviction cap with no revalidation, since eviction is silent.
- Persisting a path instead of a bookmark, or failing to balance scoped access and leaking kernel resources.
- Storing a file handle and expecting to reopen it without user activation.
- Relying on a web geolocation call with no timeout.
- Designing on the assumption that a hardware web API will reach Safari, against a written vendor position.
- Assuming a web-only capability works inside a wrapper's web view.

---

## Schools of thought (preserve disagreement)

- **Picker versus permission.** Pickers are strictly better for privacy and store review; the counterargument is genuine for gallery-management applications where full access is the feature. The bar is whether the user is picking specific items.
- **Prediction on or off.** Prediction reduces perceived latency and produces artifacts when wrong. Drawing applications generally accept the tradeoff in a transient layer; note that the tuning is content-dependent.
- **Platform palm rejection versus application classification.** Trusting the platform is simpler and inconsistent across hardware; classifying yourself is more work and more predictable.
- **Web hardware APIs at all.** One camp treats Chromium-only availability as usable given market share; the other treats a written multi-vendor objection as disqualifying for anything long-lived. **Note the fact both sides need: web views report no support, so wrapper applications are excluded regardless of browser share.**

---

## What is NOT an input-and-peripherals finding

- Whether a platform primitive already solves the problem. Route to `browser-spec`.
- How a request crosses a bridge to reach the capability. Route to `native-bridge`.
- Assistive technology, semantics, and alternative input paths. Route to `accessibility`.
- Rendering the captured stroke. Route to `graphics-programming`.
- Audio processing after capture. Route to `audio-programming`.
- Whether collecting the data is lawful or disclosed. Route to `app-privacy-compliance`.
- General platform runtime behavior. Route to `mobile-native` or `desktop-native`.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: a missing usage-description string on a reachable path, which terminates the process; branching on a media grant without the user-selected declaration so a full-access path runs against a subset; unbalanced scoped file access leaking kernel resources until relaunch.
- **major**: discarding coalesced samples on a drawing surface; requesting foreground and background location together so neither is granted; permanent audio-focus loss treated as transient; unhandled route change broadcasting audio aloud; persisted access beyond the eviction cap with no revalidation; permissions requested at launch rather than at point of use; trusting a negotiated MTU on current Android.
- **minor**: prediction committed rather than transient; pressure treated as binary; missing geolocation timeout; precise location where coarse suffices; hover as a gate.
- **nit**: naming of input handlers; gesture-recognizer organization.
- **insight**: structural -- "this feature is a picker in disguise and could drop three permissions entirely"; "input handling assumes one sample per frame throughout, so stroke quality is bounded by frame rate rather than by digitizer rate"; "the capability set here has no path on iOS, since these web APIs carry written vendor opposition and web views report no support."

Confidence: high when the trigger is a concrete API call, manifest entry, or missing declaration; medium when reasoned from feature shape. **Note the platform version gate before flagging**, since much of this behavior changed on specific releases.

---

## Process for the input-and-peripherals agent

1. **Identify the platforms and the target SDK level.** Android behavior changes gate on the target level, not the operating system.
2. **Enumerate the capabilities touched** and, for each, find the declaration, the request site, and the use site.
3. **Check declarations exist.** A missing usage string is a crash, not a denial.
4. **Check what the grant actually returns**, not merely that it was granted.
5. **Ask whether a picker would remove the permission entirely.**
6. **Check request timing**: point of use rather than launch, and note that a denial may be unrecoverable in-application.
7. **Walk input fidelity** on any drawing or gesture path: coalesced samples, prediction handling, stylus properties, tool types, palm rejection.
8. **Walk device lifecycle**: route changes, focus loss, disconnection, throttling, negotiated parameters that may be ignored.
9. **Walk persistence**: bookmarks rather than paths, balanced scoped access, eviction caps, re-grant requiring user activation.
10. **Check reach**: does the chosen web capability exist on the target browsers and inside any web view the product ships?
11. **Route to other lenses**: platform primitives to `browser-spec`; bridge mechanics to `native-bridge`; assistive input to `accessibility`; lawfulness of collection to `app-privacy-compliance`.
12. **Stay read-only.**
