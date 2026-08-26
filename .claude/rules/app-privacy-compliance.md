---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Application Privacy Compliance

A reference for reviewing the regulatory and store-policy obligations that gate shipping software: lawful basis, consent mechanics, data-subject rights, retention and deletion, and the platform privacy artifacts. Used by the `app-privacy-compliance` subagent.

**This is engineering guidance, not legal advice.** The lens flags situations that need counsel and catches code that plainly contradicts a stated policy. It does not render legal conclusions. Say so when a finding approaches that line.

Distinct from:
- **`security`**: the threat model. The axes genuinely differ. Security asks "can an attacker get this data." This lens asks **"are we allowed to have this data, did we disclose it, and can the user get it deleted."** A system can be perfectly secure and flagrantly non-compliant.
- **`web-analytics`**: event taxonomy and instrumentation correctness. We own the consent gate, the personal-data boundary, and the lawful basis for that tracking.
- **`platform-release`**: submission mechanics. We own privacy manifests and store privacy declarations as a *regulatory* matter; they own them as a submission gate.
- **`sync-and-offline`**: replication mechanics. We own why a tombstone is not an erasure.
- **`llm-app`**: model and prompt design. We own disclosure when user content reaches a vendor.

The core thesis: **compliance failures ship silently and surface as enforcement, not as bugs.** Nothing crashes. Tests pass. The defect is a mismatch between what the code does and what the policy, the store declaration, or the statute says it does.

The operational priority: **find the consent gate and check what runs before it.** An analytics or advertising SDK initialized before consent resolves has already transmitted an identifier, and consent afterward is meaningless. This single check catches more real violations than any other.

Verification markers: **[V]** verified against primary source, **[U]** unverified. **[EXPIRES]** marks facts on a known refresh cycle.

---

## Universal principles

### Consent is an ordering problem, not a UI problem

The compliance question is not whether a banner exists. It is **what executed before the user answered**.

**Flag**: SDK initialization at application launch ahead of the consent gate; tag managers loading vendors before a signal is read; a "reject" path that still fires the same initialization; identifiers minted before consent and reused after.

### Storage and access on a device is a separate obligation

The ePrivacy rule on storing or accessing information on a device is **independent of** the personal-data question, and applies to more than cookies: local storage, IndexedDB, device identifiers, and SDK fingerprinting all count. This is widely misunderstood, and "no cookies, so no banner" is not a defense.

### Data minimization is a property of code

Collecting a field "in case we need it later" is the violation itself, not a precursor to one. So is retaining it past its purpose.

**Maryland's standard is the strictest in the United States and the one most likely to be violated by ordinary code** [V]. Verbatim: a controller shall *"LIMIT THE COLLECTION OF PERSONAL DATA TO WHAT IS REASONABLY NECESSARY AND PROPORTIONATE TO PROVIDE OR MAINTAIN A SPECIFIC PRODUCT OR SERVICE REQUESTED BY THE CONSUMER."*

Why it matters: every other state ties minimization to purposes *"as disclosed to the consumer"*, a notice standard the business writes for itself. **Maryland's is an objective standard tied to the requested product or service, so consent does not cure over-collection.**

### Deletion means propagation, not a row update

An erasure request that removes the user row and leaves them in backups, logs, the analytics warehouse, the crash reporter, the support tool, and every sub-processor has not been fulfilled.

**A tombstone is not an erasure.** Systems retaining full history (event sourcing, replicated types) make this structurally harder; flag the conflict rather than assuming a delete flag suffices. See `sync-and-offline`.

### Every third-party SDK is a data processor

Adding one in a pull request adds a recipient of personal data, usually with no data-processing agreement, no sub-processor listing, and no entry in the store privacy declaration.

### The declaration must match the code

A privacy label, Data Safety form, or privacy policy that understates what the code collects is a false statement to the platform and a regulator-visible discrepancy. **This is an enforcement vector, not a paperwork problem** -- the Healthline action turned partly on a consent banner that did not actually disable tracking [V].

---

## Regulatory landscape

### GDPR

Six lawful bases, but in practice applications live on **consent**, **contract**, and **legitimate interest**. Consent must be freely given, specific, informed, and unambiguous -- which is what kills pre-ticked boxes, bundled consent, and inferred consent from continued browsing. Legitimate interest requires a documented balancing test and **is not available for most advertising use**.

Data-subject rights: access, rectification, erasure, portability, restriction, objection, and rights around automated decision-making. The engineering question for each is whether you can actually *find* all of a person's data.

Other operative obligations: data-protection impact assessments for high-risk processing, records of processing, controller-versus-processor role clarity, international transfer mechanisms, and **72-hour breach notification**. Fine tiers reach 4% of global annual turnover.

### United States: a patchwork with real structural differences

**The trackers are unreliable.** [V] The Osano tracker wrongly marks Virginia and Utah as requiring universal opt-out, lists Nebraska as undetermined, and carries entries that do not match the 2026 enactments; Wikipedia's list omits six states that do have requirements. **Verify against statute.**

**Universal opt-out mechanisms** [V], and note the distinction most summaries miss:

| State | Required | Effective | Cite |
|---|---|---|---|
| California | Yes | Regs 2023-03-29 | 11 CCR 7025 |
| Colorado | Yes | 2024-07-01 | C.R.S. 6-1-1306(1)(a)(IV)(B) |
| Connecticut | Yes | 2025-01-01 | C.G.S. 42-520(e) |
| **Texas** | **Agent-framed** | 2025-01-01 | Bus. & Com. 541.055(e) |
| Montana | Yes | 2025-01-01 | MCA 30-14-2809 |
| **Nebraska** | **Agent-framed** | 2025-01-01 | LB1074 s.11(5) |
| New Hampshire | Yes | 2025-01-01 | RSA 507-H:6 |
| New Jersey | Yes | 2025-07-15 | P.L.2023 c.266 |
| Minnesota | Yes | 2025-07-31 | Minn. Stat. **325O**.05 |
| Oregon | Yes | 2026-01-01 | ORS 646A.578(5)(c) |
| Delaware | Yes | 2026-01-01 | 6 Del. C. 12D-106(e)(1)a.2 |
| **Maryland** | **No -- permissive** | n/a | Md. Com. Law 14-4704 |
| Virginia, Iowa, Rhode Island | No | n/a | verified from statute |

**Three corrections worth carrying** [V]:
1. **Maryland is not a mandate**, contradicting most trackers. The statute says a controller *"MAY UTILIZE THE FOLLOWING METHODS"*: a conspicuous website link, **or** an opt-out preference signal. **The list is disjunctive, so the link alone discharges the duty.**
2. **Texas and Nebraska are structurally weaker.** Both use an authorized-agent frame rather than "shall honor the signal," with enumerated grounds to refuse -- including that the controller *"does not possess the ability to process the request."* Treating them as equivalent to Colorado misstates the engineering obligation.
3. **Colorado's approved list contains Global Privacy Control only.** Its statute is the cleanest illustration of the shift: the permissive "MAY ALLOW" subsection was repealed effective 2024-07-01 and replaced with **"SHALL ALLOW."**

**Unverified [U]**: Utah, Indiana, Kentucky, Tennessee. Oklahoma, Alabama, and Louisiana take effect in 2027 and were not assessed. **"No requirement" is the claim most likely to go quietly stale.**

**California browser-developer bills are a different obligation class** [V]: AB 3048 was **vetoed** 2024-09-20; AB 566, the "California Opt Me Out Act," was signed 2025-10-08 and is **operative 2027-01-01 [EXPIRES]**, binding browser developers to ship a configurable signal rather than binding businesses to honor one.

### CCPA/CPRA specifics

**"Sell" and "share" are distinct and both broad** [V]. "Sell" covers transfer *"for monetary or other valuable consideration."* "Share" covers transfer *"for cross-context behavioral advertising, **whether or not** for monetary or other valuable consideration."* **The "whether or not" clause is the point: giving data away for ad targeting still counts.**

**Sensitive personal information** now includes precise geolocation, account credentials with access codes, contents of mail and messages where the business is not the recipient, genetic data, and **neural data** [V].

**The penalty figures everyone cites are superseded** [V]. Inflation-adjusted under **Cal. Civ. Code § 1798.199.95(d)** (not § 1798.185), effective 2025-01-01 and holding through 2026, next adjustment 2027 **[EXPIRES]**:

| | Original | **Current** |
|---|---|---|
| Per violation | $2,500 | **$2,663** |
| Intentional, or minor under 16 | $7,500 | **$7,988** |
| Private right of action, per consumer per incident | $100-$750 | **$107-$799** |
| Business revenue threshold | $25,000,000 | **$26,625,000** |

**The 30-day cure period is gone** for agency and attorney-general enforcement, as of 2023-01-01 under CPRA [V]. It survives only for the private right of action on breaches.

**Enforcement is real and accelerating** [V]. Selected: Sephora $1.2M (2022), DoorDash $375K (2024), American Honda $632,500 (2025-03), Todd Snyder $345,178 (2025-05), **Healthline $1,550,000 (2025-07)**, Tractor Supply $1,350,000 (2025-09, largest agency fine), and **General Motors $12,750,000 (2026-05), the largest CCPA penalty in California history**, for selling location and driving-behavior data without consent, with data-minimization and purpose-limitation violations and a privacy policy that misrepresented the practice.

The Healthline action is the most instructive for engineers: failure to honor opt-outs, a **purpose-limitation violation for sharing article titles indicating specific medical diagnoses**, missing contractual terms with advertising third parties, and **a consent banner that did not actually disable tracking cookies** [V].

**Operational notes** [V]: the agency rebranded to **CalPrivacy** and moved its newsroom to `privacy.ca.gov` on 2026-01-26, so the old announcements page looks empty for 2026. The **Delete Request and Opt-out Platform is live** -- registered data brokers have had to process deletion requests since **2026-08-01**, accessing the platform at least every 45 days. Automated decision-making, risk-assessment, and cybersecurity-audit regulations took effect 2026-01-01, with **ADMT compliance due 2027-01-01**, first risk attestations 2028-04-01, and cyber audits phased 2028 through 2030 by revenue **[EXPIRES]**.

### Washington My Health My Data: the one with teeth

**The only US health-privacy law of its kind with a general private right of action** [V]. A violation is a **per se** Consumer Protection Act violation, exposing actual damages, treble damages capped at a $25,000 increase, plus costs and attorney's fees.

**"Consumer health data" is deliberately broad** [V]: physical or mental health status, bodily functions and symptoms, gender-affirming and reproductive care, biometric and genetic data, **precise location indicating an attempt to acquire health services**, and **inferences derived from non-health data**. That last clause is what pulls ordinary applications into scope.

**Selling requires a "valid authorization" that is stricter than consent** [V]: a standalone document naming the data, the seller, the purchaser, and the purpose, stating that goods and services cannot be conditioned on signing, with revocation instructions, a **one-year expiration**, and a signature. Copies retained six years.

**The geofencing ban is absolute**: no geofence **2,000 feet or less** from the perimeter of an in-person health care facility to identify or track consumers, collect health data, or send related advertising.

### Children

**COPPA** requires verifiable parental consent for under-13 collection. The FTC's amended rule was published **2025-04-22 with full compliance required 2026-04-22** [V]. GDPR Article 8 covers the EU equivalent. Kids-category applications face restrictions on behavioral advertising and third-party analytics.

**App-store age-verification laws are now in force and unenjoined** [V] **[EXPIRES -- actively litigating]**: Utah's App Store Accountability Act imposes provider obligations from **2026-05-06** and developer obligations from 2026-12-31, with the industry challenge voluntarily dismissed 2026-04-21. Texas S.B. 2420 took effect 2026-01-01; the Fifth Circuit lifted the hold 2026-06-25 and the Supreme Court declined to block it 2026-07-15. Texas HB 18's monitoring-and-filtering provisions remain **enjoined on Section 230 grounds**. California's Digital Age Assurance Act is operative 2027-01-01.

**Free Speech Coalition v. Paxton** (2025-06-27, 6-3) upheld Texas HB 1181 **applying intermediate scrutiny, not rational basis** [V] -- a commonly misstated holding.

### Federal

**APRA died** at the end of the 118th Congress in January 2025 without a floor vote, and no successor comprehensive bill was confirmed in the 119th -- **though that negative rests on thin sourcing** [U]. KOSA was reintroduced as S. 1748. Treat any claim of a federal comprehensive law as requiring verification.

---

## Beyond the US and EU

**India DPDP Act** [V]. Rules notified mid-November 2025 with **phased commencement: substantive obligations -- notice, consent, security safeguards, breach reporting, children's data -- do not bite until 2027-05-13 [EXPIRES]**, with consent-manager registration from 2026-11-13. **A child is under 18, stricter than COPPA's 13**, and **tracking, behavioural monitoring, and targeted advertising directed at children are prohibited outright** with no consent cure. Verifiable parental consent may use a virtual token mapped to identity details. Breach: intimate the Board without delay, detailed report within 72 hours. Cross-border uses a **blacklist** model -- transfers permitted except to restricted countries, and **none have been designated yet**. Penalties reach ₹250 crore for security-safeguard failures.

**China PIPL** [V]. Cross-border under Articles 38 to 40, with March 2024 thresholds: **under 100,000 non-sensitive records is exempt**; 100,000 to 1 million non-sensitive or under 10,000 sensitive needs standard contract filing or certification; **at or above 1 million non-sensitive or 10,000 sensitive needs a CAC security assessment**, cumulative from January 1 each year. **The security assessment is valid 3 years, not the 2 years widely repeated** -- Article 9 states it plainly, and secondary sources predate the provision. **Separate consent** (单独同意) is an explicit statutory concept for third-party sharing and cross-border transfer, distinct from general consent. Penalties are two-tier: up to RMB 1 million standard, and **up to RMB 50 million or 5% of turnover only in serious circumstances**. Free-trade-zone negative lists provide per-region carve-outs.

**Brazil** [V]. LGPD Article 52 caps fines at **2% of Brazil revenue, R$50 million per infraction**. **A dedicated children's online statute now operates alongside it**: the "ECA Digital" (Law 15.211/2025), effective March 2026, enforced by ANPD, mandating age verification, parental supervision, and advertising controls with fines to R$50 million per violation, phased through 2027 **[EXPIRES]**. Enforcement is live and aggressive: **TikTok fined R$153.7 million on 2026-08-25** for processing minors' data without valid legal basis, ordered to delete the data, default under-16 accounts to restrictive settings, suspend all Brazilian advertising, and disable livestreams and direct messaging for unregistered access; **Discord ordered to suspend Brazilian livestreaming 2026-08-12 under ECA Digital rather than the LGPD**.

**Canada** [V]. **Bill C-27 died** on prorogation 2025-01-06, so **PIPEDA still governs**. A replacement is live: **Bill C-36**, first reading 2026-06-15, currently at second reading, taking a hybrid approach of a new Act plus direct PIPEDA amendment, framed around children's data and AI chatbots **[EXPIRES -- in progress]**.

**Quebec Law 25** [V] has a genuine two-track penalty structure: **administrative penalties up to 2% of worldwide turnover or C$10 million**, and a **separate penal track up to 4% or C$25 million**, whichever is greater. Data portability has been in force since 2024-09-22.

**Australia** [V]. The Privacy and Other Legislation Amendment Act 2024 received assent 2024-12-10. **A statutory tort for serious invasions of privacy is operative law**, covering intrusion upon seclusion and misuse of private information, requiring intent or recklessness plus a seriousness threshold. **Automated-decision transparency has not yet commenced** [V]. The social media minimum-age regime is in force as Part 4A of the Online Safety Act, obliging platforms to take reasonable steps to prevent age-restricted accounts.

**Japan APPI** [V]. **The widely assumed 2025 amendment does not appear to have passed** -- the most recent development is a January 2026 *policy decision* under the triennial review, and the consolidated text is still dated April 2023. **Treat administrative surcharges, collective injunctions, under-16 rules, and relaxed AI-training consent as proposed, not enacted.** Article 28 cross-border has three routes: designated-equivalent countries (EU and UK), a recipient with a compliant system, or consent after disclosing the destination's regime.

**United Kingdom** [V]. The Data (Use and Access) Act 2025 received assent 2025-06-19. Two corrections to common belief: **it created no standalone "software updates" cookie exception** -- Schedule A1's seven paragraphs cover strictly-necessary, statistical/analytics with a means to object, appearance adaptation, and emergency assistance -- and **it did not itself raise PECR fines to UK GDPR levels**, only granting a delegated power to do so later. **Whether the cookie changes are actually in force is unverified**, since they commence by regulations not yet identified. Section 80 replaced UK GDPR Article 22 with Articles 22A to 22D. **UK adequacy was renewed 2025-12-19**, not extended again.

---

## US sectoral

**FTC Health Breach Notification Rule** [V] reaches far beyond covered entities and is the rule most likely to surprise a consumer application. Its definition of health care services expressly includes tools tracking **fitness, fertility, sexual health, sleep, mental health, genetic information, and diet**, and "breach of security" **includes unauthorized disclosure, not only intrusion** -- which is how ordinary ad-pixel sharing becomes a reportable breach.

The 2024 final rule published **2024-05-30**, effective 2024-07-29. Notify individuals **without unreasonable delay and no later than 60 calendar days**; for **500 or more individuals, FTC notice is contemporaneous with individual notice**. Civil penalty is **$53,088 per violation per day** as of 2025-01-17 **[EXPIRES -- annually adjusted]**.

Enforcement: GoodRx $1.5M (2023, first ever, with a permanent ban on sharing health data for advertising), **BetterHelp $7.8M as a Section 5 action rather than under this rule**, Premom $200,000, Cerebral $7.1M total.

**HIPAA tracking technologies** [V]: OCR's March 2024 bulletin was **held unlawful and vacated** in *AHA v. Becerra* (N.D. Tex., No. 4:23-cv-01110, 2024-06-20) as to the proscribed combination -- an IP address on an **unauthenticated public page** combined with a health-condition visit is **not** individually identifiable health information. OCR withdrew its appeal. Cite by docket number; the case name collides with an unrelated 2022 Supreme Court matter.

**VPPA is at the Supreme Court and undecided** [V] **[EXPIRES]**. A circuit split on whether "goods or services" means all or only audiovisual: the Second Circuit (*Salazar v. NBA*) and Seventh (*Gardner v. Me-TV*) read it broadly, the Sixth (*Salazar v. Paramount*) narrowly and expressly breaking with them. **Cert granted 2026-01-26; argument set 2026-10-14; not yet argued or decided.** Statutory damages are **not less than $2,500** per person plus punitive damages and fees, which is what makes pixel litigation viable.

**CIPA** [V]: section 638.51 (pen register without a court order) drives current pixel litigation, with **consent of the user** as the central defense, and section 637.2 provides a private right of action for **the greater of $5,000 per violation or treble damages with no need to show actual damages**. **SB 690, which would add a commercial-business-purpose exemption, passed the Assembly Appropriations Committee 2026-08-13 and sits on the third-reading file** -- not passed, not signed **[EXPIRES -- session ending]**.

**FERPA** binds institutions rather than vendors directly; vendors are reached through the **school official exception** requiring direct institutional control over use and maintenance of records. **There is no private right of action** (*Gonzaga v. Doe*); enforcement runs through the Department's Student Privacy Policy Office with funding withdrawal as the ultimate sanction.

**GLBA Safeguards Rule** [V] requires a Qualified Individual, written risk assessment, access controls, **encryption in transit and at rest**, **multi-factor authentication for anyone accessing any information system** unless equivalent controls are approved in writing, service-provider oversight, a written incident response plan, and an annual board report. Institutions under 5,000 consumers get only a **partial** exemption. Breach: notify the FTC **as soon as possible and no later than 30 days** for events involving **500 or more consumers**.

---

## Platform privacy requirements

### Apple

**Privacy Manifests** (`PrivacyInfo.xcprivacy`) declare collected data types, tracking domains, and **required-reason API usage** across the file-timestamp, system-boot-time, disk-space, active-keyboard, and user-defaults categories. Third-party SDKs must ship their own; **they cannot rely on the application's manifest** [V]. Enforcement has been hard since 2024-05-01.

**App Privacy labels** require accurate data-type disclosure with linked-versus-not-linked and tracking distinctions. **App Tracking Transparency** governs cross-app identifiers, and **fingerprinting remains banned regardless of ATT status** -- a point teams routinely miss when replacing the advertising identifier with a device signature.

Account deletion must be available in-app.

### Google Play

The **Data Safety** form is the declaration surface, with the same accuracy exposure. Android 13+ requires runtime notification permission. The advertising identifier returns zeros when a user opts out, and code treating that as a valid identifier is a bug. Sensitive permissions require declaration and prominent disclosure, and account deletion must be reachable from the web as well as in-app.

### The discrepancy vector

**When the declaration and the code disagree, the code is the evidence.** Review a privacy label or Data Safety form against what the SDKs in the manifest actually collect -- this is a mechanical check that finds real problems.

---

## Engineering obligations that map to code

- **Retention**: automated deletion, time-to-live on records, **log retention (logs are personal data)**, backup retention against erasure requests.
- **Data-subject access**: can you enumerate all of a person's data across primary storage, caches, warehouse, logs, crash reports, support tooling, and sub-processors? Export format and identity verification, on a 30-day clock.
- **Deletion propagation**: to sub-processors and downstream systems. **Pseudonymized data is still personal data**; true anonymization requires that re-identification be infeasible, which most "we hashed the ID" schemes do not achieve.
- **Personal data in the wrong places**: logs, analytics event properties, crash reports (stack frames, breadcrumbs, request bodies), URLs and query strings, third-party SDK payloads, **prompts sent to a model vendor**, and support tooling.
- **Data residency** and sub-processor change notification.
- **Consent records**: what was consented to, when, under which policy version, and that **withdrawal is as easy as granting**.

---

## Anti-pattern catalog

### Consent ordering
- Analytics or advertising SDK initialized before the consent gate resolves; the identifier is already transmitted.
- A tag manager loading vendors ahead of the signal.
- A "reject" path that runs the same initialization as "accept."
- Consent state cached and reused after withdrawal.
- No consent record: no timestamp, no policy version, no proof.
- Withdrawal buried deeper than granting.
- Treating a universal opt-out signal as advisory in a state that mandates it.
- Assuming Maryland mandates a signal, or assuming Texas and Nebraska mandate one the way Colorado does.

### Declaration mismatch
- Privacy label or Data Safety form omitting a data type an SDK collects.
- A privacy policy describing practices the code contradicts.
- Missing Privacy Manifest, or one omitting a required-reason API in use.
- Relying on the application's manifest to cover a third-party SDK.
- Fingerprinting introduced as an advertising-identifier replacement.
- Treating a zeroed advertising identifier as a valid identifier.

### Data lifecycle
- Collecting a field with no current purpose.
- No retention policy, or one implemented in documentation but not in code.
- Deleting the user row while leaving analytics, logs, backups, crash reports, and sub-processors intact.
- A tombstone or soft-delete treated as satisfying erasure.
- "Anonymized" data that is trivially re-identifiable.
- Log retention unbounded, with personal data in the logs.

### Third parties
- An SDK added with no data-processing agreement and no sub-processor listing.
- User content sent to a model vendor with no disclosure.
- Personal data in crash-report breadcrumbs or request bodies with no scrubbing.
- Precise location collected where coarse would serve.

### Sector-specific
- Health-adjacent inferences treated as ordinary analytics under Washington's law, which reaches inferences from non-health data.
- Location data that reveals an attempt to obtain health services.
- Under-13 collection with no verifiable parental consent, or a non-neutral age gate.
- Behavioral advertising in a kids-category application.

---

## Schools of thought (preserve disagreement)

- **Consent for everything versus legitimate-interest reliance.** Consent is defensible and conversion-hostile; legitimate interest is efficient and requires a documented balancing test that most teams never write. Regulators have narrowed its availability for advertising.
- **Privacy by design versus ship-and-remediate.** The second is common and works until an enforcement action, at which point the code archaeology is the discovery record.
- **Whether "anonymized analytics" is achievable at all.** Aggregate claims often survive re-identification analysis poorly; the honest position is usually pseudonymization plus retention limits.
- **US patchwork versus federal preemption.** Preemption would simplify engineering and would likely lower the floor; APRA's death leaves the patchwork as the operating reality.
- **Pay-or-okay walls.** Regulators have taken adverse positions; the practice persists.
- **How much scanning is enough.** Manual review does not scale; automated data-flow analysis produces noise. Most teams under-invest until an incident.

---

## What is NOT an app-privacy-compliance finding

- Attacker-oriented threat modeling, authentication, encryption strength. Route to `security`.
- Event taxonomy, funnel definitions, identity resolution correctness. Route to `web-analytics`; we own the consent gate and the personal-data boundary.
- Store submission mechanics beyond privacy artifacts. Route to `platform-release`.
- Replication and merge semantics. Route to `sync-and-offline`; we own why a tombstone is not an erasure.
- Prompt design and model selection. Route to `llm-app`; we own disclosure of user content reaching a vendor.
- **Legal conclusions.** State the engineering fact and the obligation it implicates, then say counsel is required.
- Speculation about pending legislation as though enacted.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: tracking or identifier transmission before consent in a jurisdiction requiring it; a store privacy declaration contradicted by the code in the same diff; under-13 collection with no parental-consent mechanism; selling or sharing sensitive data where a flat ban applies; health-data collection triggering Washington's private right of action with no valid authorization; a missing Privacy Manifest blocking submission.
- **major**: erasure that does not propagate beyond the primary record; no retention policy on personal data including logs; personal data in crash reports with no scrubbing; a third-party SDK added with no processing agreement or declaration update; a universal opt-out signal ignored in a mandating state; consent with no record; withdrawal harder than granting.
- **minor**: precise location where coarse would serve; over-collection of a field with a plausible but undocumented purpose; sub-processor list stale; policy version not tracked with consent.
- **nit**: wording of a consent string; placement of a privacy link.
- **insight**: structural -- "personal data enters four systems with no single deletion path, so an erasure request cannot be fulfilled without manual work"; "the retention policy exists in the privacy policy but nothing in the code enforces it"; "this feature's data collection exceeds what the requested service needs, which fails Maryland's objective standard regardless of consent."

Confidence: high when the trigger is a concrete initialization order, a declared-versus-actual mismatch, or a missing deletion path; medium when reasoned from architecture. **Lower confidence on jurisdiction-specific claims and say so** -- this domain's facts expire and the trackers are unreliable.

---

## Process for the app-privacy-compliance agent

1. **Identify the data.** What personal data does this code touch, and is any of it sensitive, health-adjacent, biometric, precise-location, or children's data? The category drives everything.
2. **Find the consent gate**, and determine what executes before it. This is the highest-yield single check.
3. **Walk the collection**: is each field necessary for the requested service, or speculative?
4. **Walk the recipients**: which third parties receive it, and does each have an agreement and a declaration entry?
5. **Walk the declaration**: does the privacy label, Data Safety form, or privacy manifest match what the code does?
6. **Walk deletion**: trace a single user's data across every store, and ask whether the erasure path reaches all of it.
7. **Walk retention**: is there a policy, and is it implemented in code rather than prose?
8. **Check the leak surfaces**: logs, crash reports, analytics properties, URLs, model prompts, support tooling.
9. **Check jurisdiction-specific triggers**: opt-out signals, health-data inferences, minors, sensitive categories with flat bans.
10. **Date every regulatory claim**, mark it as expiring, and note that trackers are unreliable and statutes should be checked.
11. **Route to other lenses** and **name the counsel boundary** where a finding turns on legal interpretation rather than engineering fact.
12. **Stay read-only.**
