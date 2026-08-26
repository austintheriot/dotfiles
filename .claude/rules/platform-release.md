---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Native Platform Release Engineering

A reference for reviewing how native application binaries reach users: code signing and identity, notarization, store submission and review, release mechanics, and desktop distribution with auto-update. Covers iOS, macOS, Android, Windows, and Linux. Used by the `platform-release` subagent.

Distinct from:
- **`ci-pipeline`**: workflow design, runners, secrets, branch protection, artifact provenance. We own the signing identity, the store gate, and the release mechanics that the pipeline invokes. Where they meet (a signing key in CI, a release job) note the seam.
- **`mobile-native`** / **`desktop-native`**: application runtime behavior. We own the path from a built artifact to a launched application on a user's machine.
- **`platform-payments`**: monetization mechanics. We own the non-payment rejection surface.
- **`build-systems`**: the build graph. We own what happens to the artifact after it is built.
- **`security`**: general threat model. We own signing-key custody and update-channel integrity.

The core thesis: **the release path fails silently at build time and catastrophically later.** A signature verifies on the build machine and fails on a clean one. A binary installs today and refuses to launch in three months when a profile expires. A timestamp omission is a warning, not an error, and kills every binary you ever signed the day the certificate expires.

**The meta-gotcha behind most of this catalog: you cannot roll back a shipped binary on any platform.** Apple's phased release cannot reverse. Google's staged rollout cannot recall. The only real lever is a server-side flag you shipped in advance. Review release code for the presence of that lever.

**The corollary worth stating to teams: the update mechanism is the highest-privilege component you ship, and its failures are silent, delayed, and unrecoverable.**

Verification markers: **[V]** verified against a primary source, **[V2]** corroborated secondary only, **[U]** unverified.

### Facts in this file expire

Roughly a dozen items below are on a known refresh cycle and **will be wrong within a year**. Marked **[EXPIRES]** inline. A reviewer citing a stale target-API level or fee structure with confidence is worse than one that says "verify this."

The recurring cycles: **Play target-API level rolls every August**; **Apple's SDK floor rolls every April**; certificate validity is on a downward ratchet (460 days as of 2026-03-01, dropping again); **Epic v. Apple and the EU DMA terms are actively litigating**; the Flathub repository GPG key expires 2027-06-14.

**When a finding depends on one of these, date it and say it needs re-verification.** Never assert a deadline as durable.

**One unresolved contradiction, carried as a known-unknown**: `debugSymbolLevel` casing differs between the Android Gradle Plugin API reference (lowercase: `'none'` / `'symbol_table'` / `'full'`) and Play Console Help (uppercase). Neither was empirically tested. Do not flag either casing as wrong.

---

## Universal principles

### Two clocks, not one

**Apple's most misunderstood mechanic**: the **certificate** is checked at signing and install time; the **provisioning profile** is re-validated **at every launch** [V].

A certificate with two years left does not save you from a profile expiring in three months. The failure mode: applications that ran yesterday refuse to launch, with no update pushed and no crash log implicating you.

This asymmetry is also why Apple extended Developer ID *profile* validity to 18 years while certificates remain **5 years** [V]. **The commonly repeated "Developer ID certs are valid 18 years" conflates the two.**

### Timestamps are what survive expiry

`codesign --timestamp` and Windows `signtool /tr` fix validity at signing time, so shipped binaries keep verifying after the certificate expires.

**Windows makes this a trap**: verbatim from Microsoft, *"If the time stamping fails, the command generates a warning"* [V]. Exit code 2 means completed with warnings. **CI checking only `errorlevel 1` ships untimestamped binaries**, and every binary ever signed goes unsigned the moment the certificate expires. Certificate validity is now capped at **460 days** (CSBR 3.10.0, effective 2026-03-01) **[EXPIRES -- on a downward ratchet]** [V], so that day comes sooner than teams expect.

Note the exception: `.pkg` installers need a **currently valid** installer certificate at run time, not merely a timestamp.

### Revocation blast radius is asymmetric

Verified from fastlane's own documentation [V]:
- **App Store and TestFlight builds survive** revocation. Apple re-signs at ingestion, so your certificate is not in the trust path.
- **Ad hoc, Enterprise, and Developer ID builds die instantly.** They carry your certificate for life.

This is why "just revoke the old certificate to clean up" is safe on one channel and catastrophic on another. Revoking an Enterprise certificate is an org-wide outage with no grace period.

### Notarization is not review

From Apple's own man page: **"Notarization is not App Review"** [V]. It scans for malicious content. A wrong entitlement, a staging URL, or a broken feature passes notarization every time.

---

## Apple

### Certificates and profiles

Types: Apple Development, Apple Distribution, Developer ID Application, Developer ID Installer, Mac Installer Distribution.

**Devices: seven families, not six** [V] -- iPod touch retains its own 100-slot pool, for 700 total. The annual reset triggers on **first sign-in to the portal**, and dismissing that prompt forfeits the reset for the year. Slots do not free mid-year.

**Ad hoc device lists are frozen at profile generation.** Adding a device requires regenerating and redistributing.

### Signing mechanics

**`codesign --deep` is deprecated for signing as of macOS 13** [V]. The man page: *"All signing options will be applied, in turn, to all nested content. **This is almost never what you want.**"* Since `--entitlements` is a signing option, **your application's entitlements get stamped onto every framework and helper**. TN2206: *"Signing with `--deep` is for emergency repairs and temporary adjustments only."*

Sign inside-out, per target. `--deep` remains correct for **verification** (`codesign -vvv --deep --strict`).

**Signing order is mandatory** [V]: *"all nested code must already be signed correctly or the signing attempt will fail."*

**Manually signing a framework signs only `Versions/Current`** -- other versions ship unsigned.

**Sideband data breaks signing.** `--strict=sideband` is now enforced automatically; `com.apple.FinderInfo` or resource-fork attributes produce *"resource fork, Finder information, or similar detritus not allowed."* Fix with `xattr -cr` or `codesign --strip-disallowed-xattrs`.

**Script signatures live in extended attributes** and most transfer tools drop them. Put scripts in `Contents/Resources` so the seal lands in `CodeResources` instead.

**Known gap in this reference**: the full enumeration of `com.apple.security.cs.*` hardened-runtime and `com.apple.security.*` App Sandbox entitlement keys, with per-key security cost, is **not covered here** -- those pages resist scraping. When a finding turns on a specific entitlement key, verify it against Apple's documentation rather than asserting from this file.

TN3125 gives the cleanest available model for profile and entitlement mismatches: a provisioning profile ties together **who** may sign, **what** they may sign, **where** it runs, **when** it runs, and **how** it may be entitled [V]. Most "it built but will not launch" defects are one of those five failing.

**Hardened runtime enables library validation**, so applications loading third-party dynamic libraries crash at launch **only in the distributed build**.

**Keychain access-control lists bind to the designated requirement** (TN2206) [V]. Rotating the signing identity produces a new requirement, macOS treats it as a different application, and users are re-prompted or silently lose stored credentials. This is the mechanism behind "we changed certificates and everyone got logged out."

TN3127 names the exact symptoms of designated-requirement drift [V]: *"the system fails to remember your choices during development"* for privacy-protected resources such as the microphone, and *"the keychain presents unexpected authorization alerts when you deploy your app through a new channel, like TestFlight."* **Shipping the same application through a different channel changes the signature, and macOS treats it as a different program.** macOS 14 added launch constraints as a further way to express limits on code.

### Notarization and stapling

`altool` uploads stopped **2023-11-01** (TN3147) [V]. Use `notarytool` with `submit`, `info`, `log`, `history`, `wait`, `store-credentials`, authenticating via App Store Connect API key or Apple ID.

**Stapling targets application bundles, disk images, and packages -- not `.zip`** [V]. `notarytool` accepts zips, but `stapler` cannot staple them; staple the `.app` inside, then re-zip.

Three stapling traps [V]:
- **Re-signing after stapling invalidates the ticket.** A late "just re-sign the DMG" step silently un-notarizes.
- **`stapler` with multiple paths only processes the last one** (a documented bug). `stapler staple *.app` exits 0 having stapled one.
- **A symlink at `Contents/CodeResources` breaks stapling.**

**Unstapled applications work online and fail offline.** Test with the network disabled, or the defect ships.

### Version numbers

TN2420, verbatim [V]: *"For iOS apps, build numbers must be unique within each release train, but they do not need to be unique across different release trains."* But **"For macOS apps, build numbers must monotonically increase even across different versions."**

**A shared versioning scheme in an iOS/macOS monorepo passes for iOS and is rejected for macOS.**

Format: digits and periods only, beginning and ending with a digit, **maximum 3 components and 18 characters**. **App extensions must match the container's version and build exactly.**

### Store submission

Guideline numbering, two commonly swapped rules [V]: **4.3(a)** is duplicate bundle IDs (one app with in-app-purchase variations, not a separate app per city); **4.2.6** is template and app-generation services.

**2.1 App Completeness** uses bare letter subsections. Verbatim: *"include demo account info (and turn on your back-end service!)"* [V].

**Sign in with Apple is not mandatory** and is never named in guideline 4.8 [V]. The requirement is an *equivalent* privacy-preserving option, with five exemptions.

**5.1.2(i)** added third-party AI disclosure (2025-11-13) [V].

**Privacy Manifests** reached hard enforcement **2024-05-01** [V]. Third-party SDKs cannot rely on the application's manifest.

**Phased release**: 1/2/5/10/20/50/100% over 7 days, pausable up to 30 days total, **with no rollback** [V]. Critically: *"apps in phased release can be manually downloaded by anyone at any time"* [V] -- **it throttles push, not pull.** If membership lapses mid-rollout the release stops and cannot resume; on reinstatement it goes to everyone at once.

**TestFlight**: 100 internal testers (App Store Connect users, 30 devices each), 10,000 external, 100 builds, **90-day build expiry** [U], with Beta App Review on the first build of a version for external groups.

**Anti-steering, United States** **[EXPIRES -- before the Supreme Court]**: guideline 3.1.1(a) verbatim -- *"These entitlements are not required for developers to include buttons, external links, or other calls to action in their United States storefront apps"* [V]. Commission is currently **zero**, but the Ninth Circuit reversed the total ban as overbroad on 2025-12-11 and Apple filed proposed rates (15/10/5%) on 2026-08-14 that are **not yet approved** [V2]. **Treat the zero-commission window as temporary.**

**European Union** **[EXPIRES -- effective 2026-10-01, actively litigating]**: the Core Technology Fee becomes a flat **5% Core Technology Commission effective 2026-10-01**, with the Initial Acquisition Fee and Store Services Fee eliminated [V]. **Japan's MSCA** took full effect 2025-12-18 [V].

**SDK requirement** **[EXPIRES -- rolls each April]**: Xcode 26 with the iOS/iPadOS/tvOS/visionOS/watchOS 26 SDK has been required since 2026-04-28 (macOS notably absent) [V].

---

## Android

### Play App Signing

The **upload key is not the app signing key**. Google's own words: the app signing key *"never changes during the lifetime of your app"* [V]. **An upload key can be reset; a self-managed app signing key cannot.** Losing the keystore before enrolling in Play App Signing burns the package name permanently.

**The fingerprint trap** [V]: Maps, Firebase, and OAuth must register **Google's app signing certificate** fingerprint, not your upload key. Debug and internal-test builds work correctly; production, re-signed by Google, fails authentication for **100% of users** -- discovered only after rollout.

### Signature schemes

v1 (JAR), v2 (Android 7, whole-archive), v3 (Android 9, rotation), **v3.1** (Android 13, rotation targeting), v3.2, v4 (Android 11, sidecar `.idsig` requiring a complementary v2/v3 signature, used by `adb install --incremental`).

Rotation caveats [V]: *"not recommended for Android 12 (API 31) and earlier"*; `checkSignatures` recognizes proof-of-rotation only on Android 13+; signing by a rotated key on API 32 or below **requires `--rotation-min-sdk-version 28`**.

**v1-only signatures are strippable** -- an attacker removes the stronger signature and old platforms accept it.

### Versioning and rollout

**`versionCode` must strictly increase, with a ceiling of 2,100,000,000** [V]. Date-plus-time-plus-ABI schemes approach it and **cannot be walked back**.

**Staged rollout percentage cannot be decreased** [V]. Verbatim: *"Users who already received the app version will remain on that version."* No recall, no downgrade. Google's own remedy is to ship a new bundle.

**`android:exported`** has no default; from API 31 the application **cannot be installed** without it [V].

**Android vitals bad-behavior thresholds** **[EXPIRES]**: 1.09% user-perceived crash rate, 0.47% ANR rate, 8% per phone model [V].

**Testing requirement is 12 testers, not 20** **[EXPIRES]** (reduced December 2024), over 14 unbroken days, for personal accounts created after 2023-11-13; organization accounts are exempt [V].

**SafetyNet Attestation fully shut down 2025-01-31** [V].

**Target API level deadlines roll annually [EXPIRES -- every August].** As of 2026-08-26: new apps and updates must target **API 36** (Android 16) by **2026-08-31**, with existing apps needing API 35 to remain discoverable, and an extension available to 2026-11-01 [V]. **Verify the current level before flagging** -- this is the single most reliably stale fact in Android release engineering.

**Play defaulted third-party store distribution to opt-in on 2026-07-22** [V]; the opt-out deadline has passed.

---

## Windows

### Authenticode

The signature lives in the attribute certificate table, a **file pointer rather than a relative virtual address**. The hash **omits the checksum and the certificate table directory**, which is why content can be appended without breaking the signature (CVE-2013-3900); the `EnableCertPaddingCheck` mitigation **remains opt-in in 2026** [V].

**Use `/tr` (RFC 3161) with `/td SHA256`, not `/t`** -- the proprietary form cannot auto-select the algorithm.

### The hardware key mandate

Since **2023-06-01**, CSBR section 6.2.7.4.2 requires the private key to be *"generated, stored, and used in a suitable Hardware Crypto Module"* meeting FIPS 140-2 Level 2 or Common Criteria EAL 4+ [V], extended to non-EV certificates.

This killed exportable `.pfx` files from public CAs, `.pfx` in CI secrets, and `signtool /f cert.pfx /p $PW`. It left untouched self-signed certificates, internal PKI, and Microsoft Store MSIX.

**Azure Artifact Signing** (renamed from Trusted Signing) reached GA **2026-01-14** at FIPS 140-3 Level 3 [V]. Certificates are valid **three days**, which works because of timestamping. You never receive the key or certificate. **No EV, ever** [V], and **no custom common name or organization**, which is a permanent constraint on your MSIX publisher distinguished name.

### SmartScreen, corrected

Microsoft's own documentation, updated 2026-08-17 [V]: **"EV certificates no longer bypass SmartScreen... Paying a premium for EV solely to avoid SmartScreen warnings is no longer justified."** This changed in 2024, and much practitioner advice predates it.

Reputation accrues over *"several weeks and hundreds of clean installs from a wide audience"*, there is no manual submission for consumer endpoints, and **renewing a certificate resets publisher reputation** [V].

### Packaging

**MSIX**: the certificate **Subject** must exactly match the manifest **Publisher** distinguished name -- *"Spaces and other punctuation must also match"* [V], failing with `0x8007000B`. **`ms-appinstaller:` has been disabled by default since December 2023** after Emotet abuse [V], removing MSIX's one-click web-install advantage.

**MSI**, verbatim [V]: *"Windows Installer uses only the first three fields of the product version... If you include a fourth field, the installer ignores the fourth field."* **Shipping 1.2.3.4 then 1.2.3.5 makes the major upgrade silently no-op.** The major field caps at 255, and major upgrades cannot cross install context (per-user to per-machine).

**WiX is at v7.0.0** (2026-04-06), not v4 or v5, and now carries an **Open Source Maintenance Fee** required at or above US$10,000 annual revenue [V]. **Squirrel.Windows is effectively abandoned** -- last release 2.0.1 in September 2020, with "Contributors Needed" as the README's first heading [V]. The successor is **Velopack**.

---

## Linux

**`apt-key` removal is later than commonly stated** [V]: present through Debian 12 and Ubuntu 24.04, removed in **Debian 13 (apt 3.0.3) and Ubuntu 25.10 (apt 3.1.6)**.

**The modern pattern moved again**: current `apt-secure(8)` prefers keys **embedded inline in `.sources`**, with the `/etc/apt/keyrings/` plus `signed-by=` file approach now the fallback for apt below 2.4 [V].

**Why `signed-by=` matters**: without it, *"all keys in the trusted keyrings are considered valid signers for this repository"* [V] -- a vendor key becomes a valid signer for the distribution's own archive.

`.asc` must be armored and `.gpg` unarmored; keyrings need mode 644. **apt 3.0 replaced GnuPG with Sequoia's `sqv`, which is stricter**, so repositories working on Debian 12 can fail on 13 [V].

**`gpgcheck` is not `repo_gpgcheck`** [V]. The former verifies package signatures, the latter metadata, and **`gpgcheck=1` alone permits a valid-signature downgrade attack**. dnf5 defaults all three to false.

**Flatpak 1.18.1 (2026-08-11) fixed a critical sandbox escape granting full host filesystem access** (GHSA-8688-9x26-hhxj) plus three high-severity issues [V]. Anything below 1.18.1 in that series is vulnerable. Flathub builds from a manifest on its own infrastructure, which is a fundamentally different trust model from uploading binaries.

**AppImage signatures are effectively decorative.** From its own documentation: reading a signature *"does not tell you whether the signature is valid or not, or whether the file has been tampered with"* [V]. Validation requires an external tool nobody runs, and AppImageKit's last release was 13, in December 2020.

---

## Desktop auto-update

**Adoption data settles the official-versus-de-facto question** (npm, last month) [V]: electron-builder 15.0M against Electron Forge 4.4M, so **builder leads 3.4x despite Forge being official**; electron-updater 10.2M; Tauri CLI 8.7M.

### Sparkle

`sparkle:version` must be **`CFBundleVersion`, not the marketing string** -- a mismatch offers downgrades or nothing at all, and the publisher sees nothing wrong. Dual verification uses EdDSA **plus** Developer ID; HTTPS is required; keep signing keys off the update server.

**Rotate the Developer ID or the EdDSA key, never both at once** [V]. Verbatim: Sparkle *"allows rotating keys by issuing a new update that changes either your Apple code signing certificate or your EdDSA keys (but not both)."* **Changing both permanently orphans every existing install**, with manual re-download by each user as the only remedy.

**Archive tools that follow symlinks break framework code signatures.** Use `ditto -c -k --sequesterRsrc --keepParent`; a generic `zip -r` produces an application permanently broken for users while the build machine's copy works fine.

### Electron

**Fuses must be flipped after packaging but before code signing** -- flipping mutates the binary and invalidates the signature. `strictlyRequireAllFuses: true` exists because new fuses in an Electron upgrade otherwise take defaults silently.

**ASAR is not encryption**; `npx asar extract` recovers everything.

Electron supports the **latest three majors** on an 8-week cadence.

---

## Anti-pattern catalog

### Apple signing and notarization
- Provisioning profile expiry: builds, signs, uploads, and installs fine, then will not launch.
- `--deep` signing, stamping the application's entitlements onto nested frameworks.
- Signing order violated: verifies on warm caches, fails Gatekeeper on a clean machine.
- Framework signed only at `Versions/Current`.
- Extended-attribute detritus producing "resource fork ... not allowed".
- Script signatures lost in transfer because they live in extended attributes.
- Attempting to staple to a `.zip`.
- Unstapled application that works online and fails offline.
- Re-signing after stapling, silently dropping the ticket.
- `stapler *.app` stapling only the last path and exiting 0.
- Hardened runtime enabling library validation, so dynamic-library loads crash only in the shipping build.
- Treating notarization as review.
- Revoking a certificate without knowing the channel's blast radius.
- Keychain access-control lists bound to the designated requirement, so certificate rotation logs every user out.
- Xcode automatic signing revoking certificates CI depends on.

### Versioning
- macOS build numbers not monotonic across versions (iOS rules applied to a macOS target).
- Extensions drifting from the container's version.
- Version strings over 18 characters or with 4 components.
- Android `versionCode` schemes approaching the 2.1 billion ceiling.
- MSI fourth version field ignored, making the major upgrade a silent no-op.

### Android
- Registering the upload-key fingerprint instead of Google's app signing certificate, failing authentication for all production users.
- Keystore loss before Play App Signing enrollment.
- Key rotation without `--rotation-min-sdk-version 28` for API 32 and below.
- v1-only signatures, which are strippable.
- `android:exported` omitted, blocking installation from API 31.
- Target SDK bump silently changing runtime behavior.

### Windows
- `signtool` timestamp failure treated as success because it exits 2, not 1.
- `/t` instead of `/tr` with an explicit digest.
- `.pfx` in CI secrets after the June 2023 hardware mandate.
- Paying for EV to bypass SmartScreen, which no longer works.
- Certificate renewal resetting publisher reputation with no plan for the gap.
- MSIX publisher and subject mismatch producing `0x8007000B`.

### Rollout and update
- Assuming phased release can be rolled back, or that it prevents manual download.
- Assuming staged rollout can be decreased or recalled.
- Sparkle `sparkle:version` set to the marketing string.
- Rotating both Sparkle keys at once, orphaning every install.
- Symlink-following archiver breaking framework signatures.
- Electron fuses flipped after signing.
- A kill switch that requires a successful launch to fetch its own configuration.
- A force-update gate that hard-blocks when its endpoint is down.
- dSYM UUID mismatch, or symbol upload failing silently behind `|| true` in CI.
- A staging endpoint shipped to production because tests also pointed at staging.
- **No rollback lever shipped at all** -- the meta-gotcha behind most of this list.

---

## Schools of thought (preserve disagreement)

- **fastlane match vs API-key automatic signing.** match gives determinism and keeps CI out of human keychains; it also deliberately shares private keys across humans, and `nuke` has real blast radius. The crux is signing determinism against key hygiene. Note `-allowProvisioningUpdates` works for both automatic and manual signing.
- **Xcode Cloud vs third-party CI vs generic Actions.** Generic Actions gives one CI for everything, but you rebuild signing yourself, which is where most failures in this catalog originate. The crux: is mobile a special-cased pipeline or one job among many?
- **Mac App Store vs Developer ID.** Sparkle ships fixes in minutes; the store takes days. The sandbox kills some capabilities. Many ship both, and **the sandbox divergence is itself a release hazard**.
- **Electron vs Tauri.** Tauri's system webview means small binaries *and* inheriting each operating system's webview quirks, which is precisely the inconsistency Electron exists to eliminate. Pay for consistency in bytes or in per-platform bug surface.
- **Release trains vs feature-based release.** Trains dominate at scale; the honest dissent is that they push risk into feature flags, which become their own outage source.
- **Staged rollout aggressiveness.** The sharpest framing: **staged rollout is a detection mechanism, not a rollback mechanism.** Its value depends entirely on whether your telemetry can decide within the window.
- **Snap vs Flatpak.** The crux is governance, not taste: snapd's store backend is single-vendor and you cannot self-host a functional alternative, while Flathub is open and multi-remote. Snap genuinely covers server and IoT cases Flatpak does not attempt.

---

## Thought leaders and sources

**Apple**: Quinn "The Eskimo!" (Developer Technical Support; forum posts are de facto canon), **Howard Oakley at eclecticlight.co** (the independent authority on Gatekeeper, notarization, and signing internals, posting roughly twice daily) [V], Jeff Johnson at lapcatsoftware.com **/articles/** [V], Patrick Wardle (Objective-See), Csaba Fitzl.

**Technotes** [V]: TN3125 provisioning profiles, TN3126 hashes, TN3127 requirements, TN3147 notarytool migration, TN2206 macOS code signing in depth (legacy archive only, roughly ten years stale), TN2420 version numbers. Modern technote URLs are JavaScript shells; content is reachable at `developer.apple.com/tutorials/data/documentation/technotes/<slug>.json`.

**fastlane is actively maintained** [V] -- 80 commits in the last 90 days and version 2.238.0 (2026-08-12). Josh Holtz is the de facto lead; Felix Krause created it but no longer runs it day to day. Real bus-factor risk remains from concentrated maintainership.

**Android**: Jake Wharton (build engineering), Romain Guy, Tor Norbye (Lint, which catches several items above).

**Release engineering at scale is a thin literature**, and worth saying plainly. Spotify's "How We Release the Spotify App" (2026-02-09) is a verified exception [V]. Widely cited Uber and Airbnb release-train articles could not be confirmed. *Continuous Delivery* (Humble and Farley) remains canon but predates app stores and has **no answer for review latency or absent rollback**. **No strong mobile-specific release-engineering book exists** -- a genuine gap.

---

## What is NOT a platform-release finding

- CI workflow structure, runner hygiene, secret scoping, branch protection. Route to `ci-pipeline`.
- Application runtime behavior after launch. Route to `mobile-native` or `desktop-native`.
- Payment and subscription mechanics. Route to `platform-payments`.
- Build graph correctness, incrementality, caching. Route to `build-systems`.
- Crash symbolication pipelines and release-health metrics. Route to `crash-and-release-health`.
- Privacy manifests as a regulatory matter. Route to `app-privacy-compliance`; we own them as a submission gate.
- Generic "use fastlane" or "switch to Tauri" advocacy without a named defect.
- Store policy speculation where the guideline text is not actually in evidence.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: timestamp omission or a CI check that only tests `errorlevel 1`; signing that will not verify on a clean machine (order violated, framework partially signed); stapling that cannot succeed (zip target, re-sign after staple); a release path with no rollback lever and no kill switch; Play fingerprint registered as the upload key; keystore custody with no Play App Signing enrollment; rotating both Sparkle keys.
- **major**: `--deep` signing in a release script; provisioning-profile expiry unmonitored; `versionCode` or `CFBundleVersion` scheme that violates the platform's monotonicity rule; MSI fourth-field upgrade no-op; MSIX publisher mismatch; `android:exported` missing; fuses flipped after signing; symbol upload failing silently; staged-rollout assumptions that treat it as reversible.
- **minor**: `/t` rather than `/tr`; extended-attribute hygiene missing from the build; EV certificate purchased for SmartScreen reasons; Linux repository configured without `signed-by=`; `gpgcheck` without `repo_gpgcheck`.
- **nit**: naming of release scripts; verbosity of signing logs.
- **insight**: structural -- "signing is reimplemented in three scripts that disagree on entitlements; centralize it"; "there is no kill switch, so the first bad release is unrecoverable for a week"; "staged rollout is configured but no telemetry decides within the window, so it is theater."

Confidence: high when the trigger is a concrete command, flag, or manifest value; medium when reasoned from pipeline shape. **Lower confidence on store policy and fee structures, and say so** -- this domain's facts expire, and several changed within the last year.

---

## Process for the platform-release agent

1. **Identify the platforms and distribution channels.** App Store, TestFlight, ad hoc, Developer ID, Mac App Store, Play, third-party Android stores, Microsoft Store, direct download, Linux repositories. Channel determines the entire catalog.
2. **Walk the signing path.** Identity source, key custody, signing order, nested content, entitlements, hardened runtime, timestamping.
3. **Check the two clocks.** Certificate expiry and profile expiry, monitored separately.
4. **Walk notarization** where applicable: submission, stapling target, re-sign ordering, offline verification.
5. **Walk versioning** against the platform's specific monotonicity rule, including extensions.
6. **Walk the rollout**: staged or phased configuration, what decides within the window, and **whether a rollback lever exists at all**.
7. **Walk the update channel** for desktop: version field semantics, key rotation policy, archive tooling, fuse ordering.
8. **Check the submission gate**: privacy manifests, SDK requirements, target API deadlines, required declarations.
9. **Check CI seams**: how keys reach the runner, whether signing failures are detectable, whether symbol upload can fail silently.
10. **Route to other lenses**: workflow structure to `ci-pipeline`; runtime behavior to `mobile-native` or `desktop-native`; payments to `platform-payments`; symbolication to `crash-and-release-health`.
11. **Date any policy or fee claim** and flag it for re-verification.
12. **Stay read-only.**
