---
paths:
  - "**/*.{swift,kt,kts,java,m,mm}"
  - "**/*.{xib,storyboard,xml}"
  - "**/AndroidManifest.xml"
  - "**/Info.plist"
---

# Mobile-Native Programming

A reference for evaluating native mobile code: iOS / macOS (Swift, SwiftUI, UIKit, AppKit, Swift Concurrency, SwiftData, Core Data) and Android (Kotlin, Jetpack Compose, Views, Kotlin Coroutines, Room). Used by the `mobile-native` subagent.

The scope: **platform-specific** bugs the language agents and general-purpose agents miss because they lack platform context. Platform lifecycle, navigation, state management, persistence, platform UI idioms (HIG / Material Design 3), App Store / Play Store review constraints, platform performance instrumentation, platform-native accessibility APIs, hardware capabilities (camera, location, sensors).

Distinct from:
- **`accessibility`**: mobile a11y semantics depth lives there. We touch (missing labels on custom controls, hardcoded font sizes breaking Dynamic Type) and defer.
- **`concurrency`**: Swift Concurrency / Kotlin Coroutines depth lives there. We touch mobile-specific cases (MainActor mismatch, ViewModel scope, background-task cancellation) and defer.
- **`audio-programming`**: Core Audio / AudioKit / AudioUnit / AAudio / Oboe route there.
- **`graphics-programming`**: Metal / MetalKit / RealityKit / Vulkan route there.
- **`performance`**: general perf there. Mobile-specific perf (main-thread contract, 16ms / 8ms budget, Baseline Profiles) is ours.
- **`i18n`**: string externalization depth there.
- **NOT React Native / Flutter**: those have their own communities. KMP and Compose Multiplatform DO count (they compile to native).

The core thesis: **mobile native review needs platform-specific knowledge that doesn't transfer from web or backend.** The lifecycle is different (apps suspend; backgrounded for minutes; killed by the OS), the UI idioms are codified (HIG / Material Design), the deployment target gates language features (iOS 17+ vs iOS 14+ have different SwiftUI vocabularies), and the platform vendor's policies matter (App Store review, Play Store policies, privacy manifests).

The empirical priority: **deployment target is the first question.** Most modern mobile features (SwiftData, `@Observable`, Compose stable, predictive back gesture, notification permissions) gate on platform version; what's a bug at iOS 17+ is unavailable on iOS 14. Identifying the target reshapes the entire review.

---

## Universal principles

### The main-thread contract is absolute

iOS: 60Hz (16.67ms / frame) on standard displays; 120Hz (8.33ms) on ProMotion. Android: 60Hz typical; 90/120Hz on flagships. Any blocking work on the main thread during render produces jank.

**Flag**: synchronous I/O on the main thread (Android throws `NetworkOnMainThreadException` in debug -- a backstop, not the design); heavy computation in view body / composable body; large JSON parsing in `viewDidLoad` blocking first paint.

### Lifecycle is not optional

Apps suspend, resume, get killed for memory, backgrounded for minutes or hours. State that doesn't survive the lifecycle is lost. Background work without explicit OS coordination is killed.

iOS lifecycle: `App` protocol, `ScenePhase` (active / inactive / background), `BackgroundTasks` framework (`BGAppRefreshTaskRequest`, `BGProcessingTaskRequest`).

Android lifecycle: Activity (created / started / resumed / paused / stopped / destroyed), Fragment (mostly avoided in Compose-first), ViewModel (survives configuration changes), Doze and App Standby restrictions.

**Flag**: state not saved across `pause` / `stop` / scene-transition; background work scheduled outside the platform's coordination (NSTimer running across backgrounding, AlarmManager outside Doze bucket); background tasks without `expirationHandler` (iOS) or proper foreground-service notification (Android).

### Platform version is the contract floor

Every API has a minimum version. `@Observable` (iOS 17+), `SwiftData` (iOS 17+), predictive back gesture (Android 14+), Photo Picker (Android 13+), CredentialManager (Android 14+). Using a feature unavailable on the deployment target is a build error or a runtime crash.

**Flag**: feature use without `if #available` / `@available` (iOS) or compileSdk version check (Android); deployment target raised without corresponding code modernization (using the old API on the new target is a missed opportunity).

### Store-policy compliance is a deploy gate

iOS App Store: Privacy Manifests (mandatory from spring 2024 for App Store), App Tracking Transparency (ATT, iOS 14.5+) for cross-app identifiers, encryption export compliance, in-app purchase mandates.

Android Play Store: notification permission (Android 13+), scoped storage (Android 11+), Play Integrity API for tampering checks, Data Safety section in Play Console.

**Flag**: missing Privacy Manifest in iOS 17+ apps (build-time blocker for App Store); private API use (App Store reject reason); missing `POST_NOTIFICATIONS` permission request on Android 13+; SharedPreferences for sensitive data (use EncryptedSharedPreferences).

---

## iOS specifics

### SwiftUI vs UIKit

The 2025 consensus: SwiftUI-first for new code. UIKit retained for non-trivial existing apps and specific cases where SwiftUI hits a wall (very performance-sensitive lists, certain animation patterns, deep PhotoKit / CallKit / CarPlay integration).

The reviewer's stance: don't impose either. Match the project's choice. Flag inconsistencies (mostly-SwiftUI app with a UIKit pocket should have a documented reason).

### SwiftUI state management

The vocabulary (matters for finding inappropriate-vs-appropriate uses):

- **`@State`**: view-local mutable state. Persists for the view's lifetime.
- **`@Binding`**: a reference to state owned elsewhere. Passed in from parent.
- **`@StateObject`**: owns an `ObservableObject`; creates once per view lifetime. (Pre-iOS 17 era.)
- **`@ObservedObject`**: references an `ObservableObject` owned elsewhere. (Pre-iOS 17 era.)
- **`@EnvironmentObject`**: looks up an `ObservableObject` via `.environmentObject(...)` on an ancestor.
- **`@Environment(\.keyPath)`**: looks up an environment value (color scheme, locale, dismiss handler).
- **`@Observable` macro (iOS 17+)**: replaces `ObservableObject`. Strictly better for new code; tracks reads at the property level.
- **`@Bindable` (iOS 17+)**: produces a binding to an `@Observable` property.

**Flag**: `@ObservedObject` where `@Observable` would do (iOS 17+); `@StateObject` in a nested view recreated on every parent re-render (use `.environment` or own at the parent); `NavigationView` in new code (use `NavigationStack` / `NavigationSplitView`); body computation doing heavy work (SwiftUI re-runs body frequently); async work inline in body (use `.task` modifier).

### Swift Concurrency and Swift 6

**Sendable conformance**: types passed across actor boundaries must be `Sendable`. Reference types with mutable state are typically not Sendable unless explicitly `@unchecked Sendable` (a safety waiver) or designed as actors.

**MainActor**: code that touches UIKit / SwiftUI views must run on MainActor. SwiftUI views are MainActor-isolated by default; the body method is MainActor.

**Strict concurrency (Swift 6, 2024)**: data-race-free Swift. The migration from Swift 5 strict-concurrency-as-warning to Swift 6 strict-concurrency-as-error is the largest language-level event in flight. Many shipping apps are still in transition.

**Flag**: non-Sendable types crossing actor boundaries; `Task { self.something() }` inside a method retaining `self` until the task completes (capture `[weak self]`); `Task.detached` used when structured concurrency was intended; missing cancellation handling in long-running `.task` (the view disappeared, task should check `Task.isCancelled` or use cancellation-aware APIs).

### Persistence

- **SwiftData (iOS 17+)**: declarative wrapper around Core Data. `@Model`, `@Query`, `ModelContainer`, `ModelContext`. Still maturing; some Core Data features missing; bugs accumulating; many apps wait for iOS 18 / 19 to adopt.
- **Core Data**: the older API. Mature; required for some features SwiftData hasn't shipped.
- **GRDB**: SQLite-first; widely used for complex query needs.
- **UserDefaults**: small key-value store; preferences only; not for any data > 1KB or sensitive.
- **Keychain**: secure credential storage. Requires the Security framework. Encrypted at rest by the OS.
- **FileManager**: filesystem; the app's container.

**Flag**: `NSManagedObject` from `viewContext` accessed in a background task (crashes); `@FetchRequest` / `@Query` predicates rebuilt inline (re-executes query every render -- hoist or memoize); `UserDefaults` for sensitive data (use Keychain); SwiftData used with iOS 16 target (build error).

### App Lifecycle and ScenePhase

iOS 13+: the App protocol + Scene composition + ScenePhase observation.

```swift
@main struct MyApp: App {
    @Environment(\.scenePhase) var scenePhase
    var body: some Scene {
        WindowGroup { ContentView() }
            .onChange(of: scenePhase) { _, newPhase in
                // active / inactive / background
            }
    }
}
```

**Flag**: state saved only on `viewWillDisappear` (the user may have backgrounded the app without that firing); long-running operations that don't handle backgrounding (must request `backgroundTask` from `UIApplication`).

### Privacy

- **App Tracking Transparency (ATT, iOS 14.5+)**: explicit opt-in for IDFA / cross-app tracking. ~75% of users opt out.
- **Privacy Manifests (mandatory spring 2024)**: declare APIs used, data collected, tracking. Build-time gate for App Store from spring 2024.
- **App Privacy Report**: user-facing report on data access. Apps should be honest about what they collect.
- **Sign in with Apple**: required as an option if other social logins are offered.

**Flag**: missing Privacy Manifest (App Store rejection); incomplete privacy declaration; logging user identifiers in crash reports.

---

## Android specifics

### Compose vs Views

The 2025 consensus is more clearly Compose-first than the iOS equivalent. Compose stable since 2021; Material 3 mature. Views remain for legacy and certain performance-critical lists (though Compose has caught up).

### Jetpack Compose state and recomposition

The vocabulary:

- **`remember { ... }`**: caches across recompositions for the same composable instance.
- **`rememberSaveable { ... }`**: caches and survives configuration changes / process death (saved to bundle).
- **`mutableStateOf(value)`**: creates a state container observable by Compose.
- **`derivedStateOf { ... }`**: a computed state that only recomposes consumers when the result changes.
- **`LaunchedEffect(key) { ... }`**: launches a coroutine; cancelled on key change or composition leave.
- **`DisposableEffect(key) { ... onDispose { ... } }`**: side effects with cleanup.
- **`SideEffect { ... }`**: runs on every successful recomposition.
- **`rememberCoroutineScope()`**: scope tied to the composable's lifecycle.

**Flag**: `remember` without keys when the parameter should re-key; `LaunchedEffect(Unit)` for effects that should re-launch on parameter change; heavy work in composable body (wrap in `remember(key) { ... }`); `LazyColumn` items without stable keys (use `key = { item.id }` to avoid positional recomposition); reading high-frequency state at the top of a deep tree (cascades; push state down, use `derivedStateOf` to filter).

### ViewModel and unidirectional data flow

The Google-recommended pattern: ViewModel + StateFlow + Repository. ViewModel survives configuration changes (handle the rotation case); StateFlow as the UI-state observable; Repository as the source of truth.

**Flag**: ViewModel held by Activity scope when navigation requires a smaller scope (use `NavBackStackEntry`'s `hiltViewModel()`); StateFlow / SharedFlow collected without `flowOn(Dispatchers.IO)` for I/O sources; `runBlocking` on the main thread.

### Navigation

Compose Navigation 2.8+ (2024): type-safe routes via `Serializable` data classes. Replaces string routes.

**Flag**: string-based routes in new code; navigation that doesn't handle the predictive back gesture (Android 14+).

### Persistence

- **Room**: SQLite wrapper; the standard. Annotations for DAO, Entity, Database; coroutine / Flow integration.
- **DataStore**: replaces SharedPreferences. Two variants: Proto DataStore (typed), Preferences DataStore (key-value).
- **SharedPreferences**: legacy. Use DataStore for new code.
- **EncryptedSharedPreferences**: for sensitive small data.
- **Keystore**: hardware-backed keys; for cryptographic operations.

**Flag**: SharedPreferences in new code (use DataStore); SharedPreferences for sensitive data (use EncryptedSharedPreferences or Keystore-backed encryption); Room queries on the main thread (use suspend or Flow).

### Kotlin Coroutines

`Dispatchers.Default` (CPU-bound), `Dispatchers.IO` (I/O), `Dispatchers.Main` (UI), `Dispatchers.Unconfined` (rare). `viewModelScope` (cancels on ViewModel clear), `lifecycleScope` (cancels on lifecycle destroy). `supervisorScope` for "one child fails, others continue."

**Flag**: `GlobalScope.launch` (no structured-concurrency parent; leaks); `runBlocking` on the main thread; missing `flowOn` for I/O sources; structured-concurrency violations (a coroutine outliving its logical scope).

### Background work and Doze

WorkManager: the modern story for deferrable background work. Foreground services for user-visible immediate work (must post a notification within 5 seconds).

Doze and App Standby (Android 6+): the OS suspends background work for apps not in the foreground. Apps not designed for this break silently.

**Flag**: AlarmManager in new code without explicit reason (WorkManager is the modern path); foreground service without the required notification (killed by OS); background polling beyond Doze restrictions.

### Privacy

- **Runtime permissions (Android 6+)**: dangerous permissions require runtime request.
- **Scoped storage (Android 11+)**: app-private storage by default; user-facing files via SAF (Storage Access Framework) or the Photo Picker.
- **Photo Picker (Android 13+)**: eliminates need for storage permission for photo selection.
- **Notification permission (Android 13+)**: must be requested; otherwise notifications silently dropped.
- **Privacy Dashboard**: user-facing report of data access.
- **Play Integrity API**: tampering detection.

**Flag**: missing notification permission request (Android 13+); storage permission requested when Photo Picker would do; missing scoped-storage handling in apps that target Android 11+.

---

## macOS / AppKit specifics

- **AppKit vs SwiftUI for Mac**: SwiftUI Mac apps have matured significantly post-Ventura. For non-trivial apps, AppKit knowledge still required (window management edge cases, menu bar, services, AppleScript integration).
- **Sandboxing**: App Sandbox entitlements, hardened runtime. Mandatory for Mac App Store; optional for direct distribution.
- **Notarization**: required for direct distribution (Gatekeeper warning otherwise).
- **Privacy permissions**: camera, microphone, screen recording, automation (AppleEvents), full disk access, accessibility (assistive features) -- each a separate permission with TCC database tracking.
- **Background launching**: LSUIElement (no Dock icon), LaunchAgent (per-user background process), LaunchDaemon (system-level).

**Flag**: Mac app missing hardened runtime; entitlements broader than needed (`com.apple.security.cs.allow-jit` only when truly needed); AppKit code accessing UI from background thread (always main thread for AppKit).

---

## Cross-cutting platform concerns

### Performance

- **iOS**: Instruments (Time Profiler, Allocations, Leaks, Energy Log, MetricKit for production telemetry).
- **Android**: Android Studio Profiler (CPU, Memory, Network, Energy), Perfetto, systrace, Macrobenchmark, Baseline Profiles, Startup Tracing.
- **Both**: main-thread discipline; 16ms / 8ms frame budget.

**Flag**: missing Baseline Profile for Android apps with significant cold-start time; no MetricKit on iOS apps for production perf telemetry; main-thread network or DB on either platform.

### Accessibility

Mobile accessibility platform APIs differ from web ARIA:

- **iOS**: VoiceOver, Dynamic Type, Reduce Motion, AccessibilityLabel, AccessibilityHint, AccessibilityValue, AccessibilityTraits, accessibilityRotor, AccessibilityRepresentation.
- **Android**: TalkBack, AccessibilityService, contentDescription, accessibility role / heading / state / actions via Semantics modifier in Compose.

**Flag** (defer to `accessibility` agent for depth): missing labels on custom controls; hardcoded font sizes that don't respect Dynamic Type / system text scale; missing reduced-motion respect on animations.

### Internationalization

- **iOS**: `String(localized:)` / Strings Catalogs (Xcode 15+), `LocalizedStringKey`, `Bundle.localizedString`. Older: `.strings` and `.stringsdict` files.
- **Android**: `@string/...` resources, `strings.xml`, `plurals.xml`, locale-specific resource directories.

**Flag** (defer to `i18n` agent for depth): hardcoded English strings in user-facing UI; missing plurals handling; hardcoded date / number formats.

---

## Anti-pattern catalog

### iOS / SwiftUI
- Body computation doing heavy work.
- `@StateObject` in a nested view recreated on parent re-render.
- Holding non-Sendable state across `await`.
- Async work inline in `body` (use `.task`).
- Missing `.task` cancellation handling.
- `NavigationView` in new code.
- `@ObservedObject` where `@Observable` would do (iOS 17+).
- Implicit animations on lists with stable IDs causing flicker.
- Background tasks without `expirationHandler`.
- Missing Privacy Manifest in iOS 17+ apps.
- `URLSession.shared` in code that needs testing (inject the session).
- Hardcoded colors vs system colors (breaks dark mode).
- Hardcoded font sizes vs Dynamic Type.
- Missing `.accessibility*` modifiers on custom controls.
- Retain cycles in escaping closures (`Task { self.something() }` without `[weak self]`).
- Core Data accessed off-context.
- Heavy work in `viewDidLoad` blocking first paint.
- `@FetchRequest` / `@Query` predicates rebuilt every render.

### Android / Compose
- `remember` without keys causing stale state.
- `LaunchedEffect(Unit)` for parameter-dependent effects.
- Heavy work in composable body.
- ViewModel held by wrong scope.
- Network on the main thread.
- Missing notification permission request (Android 13+).
- Foreground service without required notification.
- SharedPreferences for sensitive data.
- Hardcoded `android:label` instead of `@string` resource.
- `Toast` for accessibility-critical info.
- Missing `contentDescription` on custom Composable.
- Hardcoded densities vs adaptive layouts.
- Missing Baseline Profile.
- Excessive recomposition (high-frequency state at top of tree).
- `LazyColumn` items without stable keys.
- `GlobalScope.launch`.
- `runBlocking` on the main thread.

### Cross-platform mobile
- Polling network in foreground (battery cost).
- Animations without reduced-motion respect.
- Missing platform-specific dark mode.
- Hardcoded English strings.
- Hardcoded server URLs vs build-config.
- Logging PII in production builds.
- Missing crash reporting.
- Store-policy non-compliance (private API use, missing privacy declarations, missing required permissions).

---

## Modern shifts (2023-2025)

### iOS
- SwiftUI maturity (iOS 17 / 18).
- Swift 6 strict concurrency.
- `@Observable` replacing `ObservableObject` (iOS 17+).
- SwiftData (iOS 17+), still maturing.
- App Intents replacing SiriKit (iOS 16+).
- Privacy Manifests (mandatory spring 2024).
- visionOS (2024).
- Liquid Glass redesign (iOS 26, 2025).
- Apple Intelligence (iOS 18+).
- TipKit (iOS 17+).
- SwiftCharts (iOS 16+).
- Swift Macros (Swift 5.9+, used in `@Observable`, `@Model`, `@Query`).

### Android
- Jetpack Compose stable maturing.
- Material Design 3 / Material You.
- Kotlin Multiplatform stable; Compose Multiplatform growing.
- Predictive back gesture (Android 14+).
- Foldables / large screens; `WindowSizeClass`.
- Photo Picker (Android 13+).
- CredentialManager (Android 14+).
- Health Connect (Android 14+).
- Privacy Sandbox on Android.
- Baseline Profiles standardized.
- Type-safe navigation in Compose (2024).

### Cross-platform regulatory
- EU Digital Markets Act (iOS 17.4+ in EU, 2024): alternative app marketplaces, NFC access, browser engine choice.
- App Store / Play Store IAP rulings: anti-steering weakened in some jurisdictions.
- Privacy regulation (CCPA, GDPR, India DPDP).

---

## Schools of thought (preserve disagreement)

- **SwiftUI-first vs UIKit-still**: SwiftUI consensus for new code; UIKit retained for non-trivial existing apps and specific cases.
- **Compose vs Views**: more clearly Compose-first in 2025.
- **Architecture (iOS)**: vanilla SwiftUI / MVVM / TCA / VIPER. TCA adds complexity; vanilla suffices for many apps.
- **Architecture (Android)**: MVVM (Google-recommended) / MVI / Clean Architecture. Mismatched complexity is the bug.
- **KMP / Compose Multiplatform vs native everywhere**: shop-specific. Native-everywhere prioritizes platform-feel; KMP shares logic, CMP extends to UI.
- **Hot vs cold streams (iOS)**: Combine deprecated-feeling; AsyncSequence / Observable for new code.
- **Hot vs cold streams (Android)**: StateFlow / SharedFlow / Flow choice depends on state-vs-event modeling.

---

## What is NOT a mobile-native finding

- React Native / Flutter / Cordova / Capacitor code (different ecosystems).
- Web view-shells / Cordova hybrid apps unless touching native layers.
- Backend code that just happens to serve mobile clients.
- General language-level concerns (Swift type design, Kotlin idioms): route to language agents.
- Deep accessibility, concurrency, audio, graphics, perf, i18n -- route to those agents.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: missing Privacy Manifest in iOS 17+ App Store submission (deploy gate); App Store / Play Store rejection cause (private API use, missing required permissions); data corruption from Core Data context misuse; foreground service without required notification (killed by OS); platform-specific crash with reachable trigger.
- **major**: ViewModel scope mismatch causing data loss on rotation / navigation; `@StateObject` recreated on parent re-render losing state; `runBlocking` on main thread; missing notification permission on Android 13+; SharedPreferences for sensitive data; missing Baseline Profile on apps with significant cold-start time; `LazyColumn` items without stable keys causing perf issues at scale; missing `.task` cancellation.
- **minor**: `@ObservedObject` where `@Observable` would do; hardcoded colors / fonts that break dark mode / Dynamic Type; SharedPreferences in new Android code (use DataStore); `URLSession.shared` in untestable code paths.
- **nit**: `NavigationView` in new code; missing build-flavor for URLs.
- **insight**: structural -- "this codebase mixes SwiftUI and UIKit without a clear boundary; consider establishing the migration plan"; "the architecture is heavyweight TCA for an app that vanilla SwiftUI + @Observable would serve"; "the Compose stability story would benefit from `@Stable` annotations on these models."

Confidence: high when the trigger is concrete and platform-specific; medium when reasoned from architecture.

---

## Process for the mobile-native agent

1. **Identify the platform**: iOS, macOS, Android, or mixed (KMP). Different review catalogs apply.
2. **Identify the UI framework**: SwiftUI / UIKit / AppKit / Compose / Views. State-management and lifecycle catalogs differ.
3. **Identify the deployment target**: iOS 17+ vs iOS 14+ enables different vocabularies; Android 13+ vs 8+ same. Findings gate on the target.
4. **Walk lifecycle**: state preserved across app suspension / resume / kill? Background work coordinated with OS? Foreground service has notification?
5. **Walk state management**: observable types correct for platform version? Scope appropriate? Recomposition / re-render efficient?
6. **Walk persistence**: appropriate storage for the data type? Off-context access? Sensitive data in plaintext storage?
7. **Walk privacy / store policy**: Privacy Manifest? Notification permission? Required entitlements? No private API use?
8. **Walk concurrency**: Sendable conformance? MainActor where required? Cancellation handling? Coroutine scope correct?
9. **Walk performance**: main-thread work? Baseline Profile? Adaptive layouts?
10. **Walk accessibility** at the platform-API level (semantic labels on custom controls, font scaling), defer depth to `accessibility`.
11. **Route to other lenses**: deep concurrency → `concurrency`; deep a11y → `accessibility`; audio → `audio-programming`; graphics → `graphics-programming`; i18n → `i18n`.
12. **Stay read-only.**
