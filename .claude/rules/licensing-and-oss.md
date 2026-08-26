---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Software Licensing and Open-Source Compliance

> **THIS IS ENGINEERING GUIDANCE, NOT LEGAL ADVICE.**
>
> Nothing in this file is a legal opinion, and this agent is not a lawyer. Its job is to
> **spot the situations that need counsel and describe them precisely enough that counsel can
> act quickly** -- not to render legal conclusions.
>
> Three rules bind every finding:
>
> 1. **Report mechanism, not verdict.** Write "AGPL-3.0 in a network-served backend triggers
>    §13 source-offer obligations if the code is modified; this needs counsel review before
>    ship," never "this is illegal" or "you are safe."
> 2. **Never say a use is permitted.** The agent can say an obligation *exists* and what
>    satisfies it mechanically (ship this file, add this notice). It must not clear a use.
>    "No finding" is not a legal clearance, and the agent must say so when asked.
> 3. **Facts here expire.** Licenses get changed by vendors, cases get decided, tools get
>    abandoned. Every claim below carries a verification marker. Re-verify anything
>    load-bearing before relying on it.
>
> Jurisdiction matters and is almost never stated in a code review. Copyleft scope, the
> enforceability of a license as a contract, and moral rights all vary by country. When a
> finding depends on jurisdiction, say so.
>
> **Verification legend.** `[V]` verified against a primary source during research (license
> text, vendor legal page, standards body, court docket, repository metadata).
> `[V-2026-08]` verified as of that date for facts known to drift. `[U]` unverified --
> plausible, commonly repeated, but not confirmed; treat as a lead, not a fact.
> `[DISPUTED]` genuine disagreement among competent people.

**A note on this file's size.** At roughly 90KB it is the largest reference in the roster, because its value is the **verbatim clause text** -- an obligation you can quote is actionable, and a paraphrase of it is not. Case law and sources are split into `~/.claude/rules/licensing-cases.md` to keep this loadable. If you need only the mechanics, sections 2, 3, 4, and 8 are the operative ones; section 1 is the per-license reference to consult for a specific dependency.

---

## 0. How to use this file in review

**The agent's output is a triage list, not a compliance sign-off.** Severity maps to how much
of someone's time the finding deserves:

| Severity | Meaning | Examples |
|---|---|---|
| **blocker** | Ships an obligation the product structurally cannot meet, or a license the org bans | AGPL in a proprietary SaaS backend; GPL-3.0 in an iOS app; LGPL statically linked into a signed mobile binary |
| **major** | Real obligation not currently satisfied | Missing Apache-2.0 NOTICE propagation; no attribution surface at all; `unknown` license bucket not failing CI |
| **minor** | Obligation satisfied sloppily; latent risk | License text present but not the version shipped; SPDX identifier deprecated form |
| **insight** | Worth knowing, no action now | A dependency's upstream is drifting toward a relicense |

**What the agent must escalate to counsel rather than resolve:**
- Any copyleft license in a distributed proprietary product
- Any source-available license (BUSL, SSPL, ELv2, FSL, RSAL, Commons Clause, PolyForm)
- Any license the scanner reports as `unknown`, `NOASSERTION`, or `LicenseRef-*`
- Any dependency that changed license between versions
- Any question of the form "is this a derivative work?" -- that is a legal conclusion
- Vendored or copy-pasted code with unclear provenance
- Anything touching patents, trademarks, or a CLA/assignment

---

## 1. The license landscape

### 1.1 Permissive

**MIT** (SPDX: `MIT`) `[V]`
The whole obligation is one sentence: *"The above copyright notice and this permission notice
shall be included in all copies or substantial portions of the Software."* `[V]`

The trap: **"all copies" includes binaries.** A compiled app, a Docker image, and a minified
JS bundle are all copies. Shipping MIT code with the notice only in your source repo does not
satisfy this. MIT has **no explicit patent grant** -- Blue Oak Council rates it Silver rather
than Gold for exactly this reason `[V]`.

**BSD-2-Clause / BSD-3-Clause** (`BSD-2-Clause`, `BSD-3-Clause`) `[V]`
Both require notice retention in source (cl. 1) and, critically, **in binary form**: cl. 2
requires reproducing the notice *"in the documentation and/or other materials provided with
the distribution"* `[V]`. That phrase is the explicit hook for an About box or a
`THIRD-PARTY-NOTICES.txt`. BSD-3 adds cl. 3, a no-endorsement clause -- a trademark-flavored
restriction, not a copyright one.

**BSD-4-Clause** (`BSD-4-Clause`) -- **the advertising clause** `[V]`
Clause 3: *"All advertising materials mentioning features or use of this software must display
the following acknowledgement: This product includes software developed by the organization."*
`[V]`

Why it matters: this is an *additional restriction*, so it is **GPL-incompatible**, and the
obligation is absurd at scale (the famous case being ~75 such acknowledgements accumulating in
BSD distributions). UC Berkeley retracted it for its own code in 1999 `[U]`, but the clause
survives in the wild. **Finding: any `BSD-4-Clause` dependency is a genuine flag** -- both for
GPL incompatibility and because the marketing obligation is one nobody will actually honor.

**Apache-2.0** (`Apache-2.0`) `[V]` -- the one people get wrong
Four redistribution conditions in §4 `[V]`:
- **4(a)** give recipients a copy of the License
- **4(b)** cause modified files to carry prominent notices stating you changed them
- **4(c)** retain all copyright, patent, trademark, and attribution notices from the source
- **4(d)** **if the Work includes a NOTICE file, include its attribution notices** in one of:
  a NOTICE file distributed with the derivative work, within accompanying documentation, or
  *"within a display generated by the Derivative Works"* `[V]`

That last phrase is the hook that legitimizes an in-app About box as a compliance surface
`[V]`.

**§3 patent grant** is Apache-2.0's real value over MIT: an express, perpetual, irrevocable
patent license from each contributor -- which **terminates if you initiate patent litigation
alleging the Work infringes** `[V]`. **§6 grants no trademark rights** `[V]`.

**The NOTICE file is the single most-violated permissive obligation.** It is separate from the
LICENSE file, it propagates transitively, and generic license-aggregation tooling frequently
drops it because it is not the license text.

**ISC** (`ISC`) `[V]` -- functionally MIT, shorter, no patent grant. Blue Oak Silver.

**Zlib** (`Zlib`) `[V]` -- three conditions: don't misrepresent origin; mark altered versions;
and *"This notice may not be removed or altered from any source distribution."* `[V]`
**Note the asymmetry**: condition 3 binds *source* distribution only, so Zlib imposes no
binary-attribution requirement -- unlike MIT and BSD. Useful to know when triaging.

**Public-domain dedications -- and the patent problem**

**CC0-1.0** (`CC0-1.0`): §4(a) says it outright: *"No trademark or patent rights held by
Affirmer are waived, abandoned, surrendered, licensed or otherwise affected by this
document."* `[V]` §3 provides a fallback license if the waiver fails in a given jurisdiction
`[V]`.

That express patent carve-out is why some orgs restrict CC0 **for code** -- a contributor can
dedicate copyright and still assert patents.

**Three corrections to widely-repeated claims:**
- **The patent objection is the FSF's, not Google's.** FSF, verbatim: *"For works of software it
  is not recommended, as CC0 has a term expressly stating it does not grant you any patent
  licenses... If the developer is refusing users patent licenses, the program is in effect a
  trap for users and users should avoid the program."* `[V-2026-08]` FSF still rates CC0
  GPL-compatible and free. **Do not attribute the patent argument to Google.**
- **Google does not restrict CC0.** Its third-party policy lists CC0 and Unlicense under
  **"unencumbered" (permitted)**. The caveat is *procedural* -- public-domain status requires a
  case-by-case legal review before check-in -- not patent-based `[V-2026-08]`.
- **Creative Commons itself says CC0 is fine for software**, even while recommending against
  other CC licenses for code `[V]`.
- Fedora's CC0-for-code policy `[U]` -- **could not verify** (site behind anti-bot protection;
  `fedora-license-data` raw paths 404'd). Fedora is commonly cited as restricting CC0 for code
  over the patent question. **Do not publish this as verified.**

**OSI approval status** -- verified directly from SPDX `licenses.json` (`isOsiApproved`)
`[V-2026-08]`. The first row is the one people get wrong:

| License | SPDX ID | OSI-approved | FSF libre |
|---|---|---|---|
| **Unlicense** | `Unlicense` | **YES** | Yes |
| 0BSD | `0BSD` | Yes | -- |
| WTFPL | `WTFPL` | **No** | Yes |
| CC0-1.0 | `CC0-1.0` | **No** | Yes |
| BSD-4-Clause | `BSD-4-Clause` | **No** | Yes |
| BSD-3-Clause-Clear | `BSD-3-Clause-Clear` | **No** | Yes |
| SSPL-1.0, Elastic-2.0, BUSL-1.1, FSL-1.1-* | -- | **No** | -- |

**Unlicense IS OSI-approved** -- the most commonly mis-stated item in this table. It still has
no patent grant and no trademark mention `[V]`. CC0's non-approval traces to a 2012 withdrawal
from OSI review `[U on the withdrawal history; the status itself is verified]`. Blue Oak rates
WTFPL **Lead**, its bottom tier `[V]`. All of these raise the same "is a public-domain
dedication even effective in this jurisdiction?" question that CC0's §3 fallback exists to
answer.

**Blue Oak Council ratings** `[V-2026-08]` -- a useful practitioner sanity check. Tiers are
**Gold / Silver / Bronze / Lead**. Gold currently holds essentially one entry
(`BSD-2-Clause-Patent`); Apache-2.0, MIT, ISC are **Silver**; BSD-3-Clause is **Bronze**;
Beerware, WTFPL, Artistic-1.0 are **Lead**. The **Blue Oak Model License 1.0.0** is a
plain-language permissive license with an **explicit patent grant and a 30-day cure period**
-- both things MIT lacks `[V]`.

### 1.2 Weak copyleft

**LGPL-2.1 vs LGPL-3.0 -- and what the text actually says about linking**

Folklore says "dynamic linking is fine, static linking is not." **The license text does not say
that.** What it actually says:

**LGPL-2.1 §6** `[V]`: you may combine the Library with your own work and distribute under
terms of your choice, *provided those terms permit modification of the Library and reverse
engineering for debugging such modifications*, and provided you do one of (a)-(e). Option (a)
is shipping source or object code sufficient **to relink**; option (b) is a **shared library
mechanism**; (c) a three-year written offer; (d) equivalent access; (e) verified prior receipt.

**LGPL-2.1 §5** `[V]` defines the header-file threshold: an object file using only *"numerical
parameters, data structure layouts and accessors, and small macros and small inline functions
(ten lines or less in length)"* is unrestricted. Above that line, the object file is a
derivative of the Library.

**LGPL-3.0 §4(d)** `[V]` makes the same choice explicit and binary:
- **(d)(0)** convey Minimal Corresponding Source **plus** your application's code *"in a form
  suitable for, and under terms that permit, the user to recombine or relink"*, or
- **(d)(1)** *"Use a suitable shared library mechanism... (a) uses at run time a copy of the
  Library already present on the user's computer system, and (b) will operate properly with a
  modified version of the Library that is interface-compatible"* `[V]`

**So the real obligation is user substitutability, not the linker flag.** Dynamic linking is
the *easy way* to satisfy it, because it satisfies (d)(1) automatically. Static linking is
permitted -- but only if you ship enough to let a user relink, which means shipping your
object files.

**LGPL-3.0 §4(e)** additionally imports GPL-3.0 §6 **Installation Information** where that
would otherwise be required `[V]` -- which is what makes LGPL-3.0 strictly harder than
LGPL-2.1 on locked-down devices.

**MPL-2.0** (`MPL-2.0`) `[V]` -- file-level copyleft, and the pragmatic choice
- **1.4 "Covered Software"**, **1.10 "Modifications"**: any file that adds to, deletes from, or
  modifies the contents of Covered Software, **or any new file containing Covered Software**
  `[V]`
- **§3.1/3.2**: distribute Covered Software in source form under MPL; executables may go under
  other terms provided recipients' MPL source rights are not restricted `[V]`
- **§3.3 "Larger Works"**: you may distribute the larger work under your own terms `[V]`
- **1.12 "Secondary License"** = GPL-2.0, LGPL-2.1, AGPL-3.0, or later versions `[V]`
- **Exhibit B**: *"This Source Code Form is 'Incompatible With Secondary Licenses', as defined
  by the Mozilla Public License, v. 2.0."* `[V]` -- an opt-in flag the initial author sets to
  block GPL relicensing

**MPL-2.0 is linking-agnostic** -- the text draws no static/dynamic distinction, and Mozilla's
FAQ confirms new files containing no MPL code are not Modifications `[V]`. This makes MPL-2.0
materially safer than LGPL in statically-linked ecosystems (Rust, Go, mobile). It is the
reason OpenTofu and much HashiCorp SDK code sit at MPL-2.0.

**EPL-2.0** (`EPL-2.0`) `[V]`
- GPL compatibility is **opt-in, not automatic**: §3.2(a) permits distribution under a
  Secondary License only where *"the initial Contributor attached to the Source Code the
  notice described in Exhibit A"* `[V]`. Do not assume EPL-2.0 code is GPL-combinable --
  check for the notice.
- **§4** imposes a **commercial distributor indemnity**: a Commercial Contributor must defend
  and indemnify other Contributors against claims arising from its own acts `[V]`. That is an
  affirmative liability most engineers never notice.

**CDDL-1.0/1.1** (`CDDL-1.0`) `[V]` -- file-level copyleft, GPL-incompatible
The incompatibility mechanism: **§3.1** requires Covered Software be distributed *"only under
the terms of this License"*, while **§3.4/3.5** let you license executables under terms of
your choice `[V]`. GPL requires the *entire combined work* to be under GPL; CDDL requires its
files to stay CDDL. Neither can yield. **This is the ZFS-on-Linux problem** and the reason ZFS
ships as a separately-built kernel module rather than in-tree `[U for the ZFS specifics]`.

### 1.3 Strong copyleft

**GPL-2.0-only vs GPL-2.0-or-later -- and why the distinction is load-bearing**

SPDX deprecated the bare `GPL-2.0` identifier in **License List v3.0** in favor of
`GPL-2.0-only` and `GPL-2.0-or-later` `[V]`. This is not pedantry:
- **`GPL-2.0-only`** cannot be combined with GPL-3.0 code. Ever. The Linux kernel is
  GPL-2.0-only, which is precisely why kernel code cannot absorb GPL-3.0 code.
- **`GPL-2.0-or-later`** may be used under GPL-3.0 terms, so it *can* combine with GPL-3.0.

**Finding: a dependency declared as bare `GPL-2.0` is under-specified.** The agent should flag
it and ask which it is, because the compatibility answer differs completely.

**GPL-2.0 obligations** `[V]`
- **§3**: distributing object code requires one of (a) accompany with complete corresponding
  source; (b) **a written offer, valid for at least three years**, to give any third party the
  source for no more than the cost of physical distribution; or (c) pass along the offer you
  received (noncommercial only) `[V]`
- **"Complete source code"** is defined broadly: *"all the source code for all modules it
  contains, plus any associated interface definition files, plus the scripts used to control
  compilation and installation of the executable"* `[V]` -- **build scripts and toolchain
  configuration are in scope**, which is what most "we published a tarball" attempts miss
- **§2 "mere aggregation"**: *"mere aggregation of another work not based on the Program with
  the Program... on a volume of a storage or distribution medium does not bring the other work
  under the scope of this License"* `[V]`
- **§7 -- the Apache-2.0 incompatibility mechanism** `[V]`: *"If... conditions are imposed on
  you... that contradict the conditions of this License, they do not excuse you from the
  conditions of this License. If you cannot distribute so as to satisfy simultaneously your
  obligations under this License and any other pertinent obligations, then as a consequence
  you may not distribute the Program at all."* `[V]`

**GPL-3.0 additions** `[V]`
- **§3 anti-DRM**: *"No covered work shall be deemed part of an effective technological measure
  under any applicable law fulfilling obligations under article 11 of the WIPO copyright
  treaty"* `[V]`
- **§6 anti-tivoization**: for a **"User Product"** (consumer property, or anything designed for
  incorporation into a dwelling), conveying object code requires **"Installation
  Information"** -- the *"methods, procedures, authorization keys, or other information
  required to install and execute modified versions"* `[V]`. **Trigger nuance:** §6 applies
  when conveyance *"occurs as part of a transaction in which the right of possession and use
  of the User Product is transferred"* `[V]`. An app-store download is arguably a separate
  transaction from the device purchase, so §6 likely does **not** bite for ordinary apps but
  **does** for preinstalled software `[V, reasoning flagged as analysis]`.
- **§10 "no further restrictions"**: *"You may not impose any further restrictions on the
  exercise of the rights granted or affirmed under this License."* `[V]` -- **this, not §6, is
  the operative App Store conflict** (see §4.4)
- **§11 patent provisions**, including the bar on discriminatory patent licenses `[V]`
- **§1 "System Libraries"** carve-out from Corresponding Source `[V]`

**AGPL-3.0** (`AGPL-3.0-only` / `AGPL-3.0-or-later`) -- read §13 precisely `[V]`

Verbatim: *"Notwithstanding any other provision of this License, **if you modify the
Program**, your modified version must prominently offer all users interacting with it remotely
through a computer network (if your version supports such interaction) an opportunity to
receive the Corresponding Source of your version by providing access to the Corresponding
Source from a network server at no charge, through some standard or customary means of
facilitating copying of software."* `[V]`

**Three things engineers get wrong:**

1. **§13 is conditioned on modification.** Running an *unmodified* AGPL program as a network
   service does not trigger §13 by its own terms `[V]`. §0 defines "modify" as *"to copy from
   or adapt all or part of the work in a fashion requiring copyright permission, other than
   the making of an exact copy"* `[V]`.
2. **But "unmodified" is a narrower safe harbor than it sounds.** Configuration is usually not
   modification; patches, forks, and vendored changes are. And AGPL's §13 covers
   *"Corresponding Source **of your version**"*, which raises the scope question of what
   counts as part of your version. **This is exactly a counsel question.**
3. **"Internal use only" is not automatically safe.** §13 says *"all users interacting with it
   remotely through a computer network."* If employees, contractors, or another business unit
   interact with a modified AGPL service over a network, that is a colorable trigger. The
   common belief that AGPL only bites for public SaaS is **not** what the text says.

§13 also carries the **GPL-3.0 combination permission**: AGPL-3.0 and GPL-3.0 works may be
linked into a single combined work, each part keeping its own license `[V]`.

**Practical note:** Google bans AGPL outright for its own codebase `[V-2026-08]`. Many orgs
follow. If the org has a policy, AGPL is usually a blocker regardless of the modification
analysis.

### 1.4 Source-available (NOT open source)

None of these are OSI-approved, and that matters concretely: OSI approval is what most
corporate policies, `deny.toml` allowlists, and Linux-distro inclusion rules key on. A
source-available license fails those gates by default, and vendors' "still open" marketing does
not change the gate.

**BUSL-1.1** (`BUSL-1.1`) `[V]` -- MariaDB-stewarded, still current, no 1.2 `[V-2026-08]`
Parameterized: **Licensor**, **Licensed Work**, **Additional Use Grant**, **Change Date**,
**Change License**. Grant is *"the right to copy, modify, create derivative works,
redistribute, and make **non-production use**"* `[V]`.

Two constraints most summaries miss `[V]`:
- **The Change Date is capped at four years**: conversion happens *"on the Change Date, or the
  fourth anniversary of the first publicly available distribution, whichever comes first."*
  A vendor cannot set a 10-year window.
- **The Change License must be GPL-2.0-compatible** (GPL-2.0-or-later, or a compatible
  license). So BUSL code *always* eventually becomes open source. HashiCorp picks MPL-2.0;
  Akka and Redpanda pick Apache-2.0.

**The rolling clock is per-version.** Each release has its own Change Date. This means a
BUSL repo contains a mix of still-restricted and already-converted versions -- a real
compliance subtlety.

**SSPL-1.0** (`SSPL-1.0`) `[V]` -- the most aggressive of the group
§13: if you make the Program's functionality available to third parties as a service, you must
release the **"Service Source Code"** -- defined to include not just the Program but *"all
programs that you use to make the Program or modified version available as a service,
including... management software, user interfaces, application program interfaces, automation
software, monitoring software, backup software, storage software and hosting software"* `[V]`.

That enumeration is the point: it reaches your entire operational stack. **Effectively
unusable in a commercial service.** Google bans it `[V-2026-08]`.

**Elastic License 2.0** (`Elastic-2.0`) `[V]` -- three limitations `[V]`:
1. no providing the software to third parties as a hosted or managed service exposing a
   substantial set of its features
2. no circumventing license-key functionality
3. no removing or obscuring licensing/copyright notices

**Functional Source License** (`FSL-1.1-ALv2`, `FSL-1.1-MIT`) `[V]`
**SPDX identifier correction:** the identifiers are **`FSL-1.1-ALv2`** and **`FSL-1.1-MIT`**.
There is no `FSL-1.1-Apache-2.0` in SPDX, even though Sentry's own LICENSE.md labels itself
that way `[V-2026-08]`. Use the SPDX form in metadata.

Created by Sentry (Chad Whitacre), announced 2023-11-17 `[V]`. **Two-year** change window with
an *"irrevocable"* grant of Apache-2.0 (or MIT) *"effective on the second anniversary"* `[V]`.
Restricts **"Competing Use"**; permits internal use, non-commercial education and research, and
professional services `[V]`. Explicitly designed as a fix for BUSL's four-year window and
parameter sprawl `[V]`.

**Fair Source** (fair.io) is the umbrella movement; qualifying licenses are FSL, Fair Core
License (FCL), and BUSL `[V-2026-08]`. Adopters include Sentry, GitButler, Keygen, PowerSync,
CodeCrafters, Typebot, Qlty, Tuist, and Liquibase `[V]`.

**Others**: **Confluent Community License v1.0** `[V]` (bars competing SaaS; note **Apache
Kafka itself remains Apache-2.0** `[V]`); **RSALv2** (Redis) `[V]` -- **not in the SPDX license
list**, so scanners bucket it as unknown `[V-2026-08]`; **CockroachDB Software License**
(proprietary, replaced BUSL in v24.3.0+) `[V]`.

**Commons Clause** `[V]` -- not a license, a **rider** bolted onto one
Removes *"the right to Sell the Software"*, where "Sell" is defined to include *"fees for
hosting or consulting/support services"* `[V]`. Drafted by **Heather Meeker** `[V]`. Its own
FAQ concedes the result is **not open source** `[V]`. Google bans it `[V-2026-08]`.

**PolyForm** licenses (Noncommercial, Perimeter, Shield, Strict, Internal Use, Small Business,
Free Trial, Countdown) `[V-2026-08]` -- the project states outright it *"is not Open Source or
free software."*

### 1.5 Creative Commons for non-code assets

**Creative Commons recommends against CC licenses for software** `[V]`, in its own words,
because they *"do not contain specific terms about the distribution of source code"*, do not
address patents, and are not compatible with major software licenses. CC0 is the exception CC
itself endorses for software `[V]`.

For **assets** (images, audio, docs, icon sets), CC is everywhere and creates real obligations:

- **CC-BY-4.0**: attribution required -- creator name, copyright notice, license notice,
  disclaimer notice, a URI to the material where practicable, **and an indication if you
  modified it** `[V]`. Cropping or recoloring an icon is a modification you must flag.
- **CC-BY-SA-4.0**: §3(b) -- adaptations must be licensed under the same or a **"BY-SA
  Compatible License"** `[V]`. **This is copyleft for assets.** A UI derived from a CC-BY-SA
  illustration can pull a ShareAlike obligation onto your design work. There is a one-way
  compatibility path from CC-BY-SA-4.0 to GPL-3.0 `[V]`.
- **CC-BY-NC-***: the NonCommercial restriction makes these **unusable in a commercial
  product**, full stop. Google bans all non-commercial licenses `[V-2026-08]`. The definition
  of "commercial" is famously fuzzy, which makes it worse, not better.
- **CC-BY-ND**: no derivatives -- cannot resize, recolor, or adapt.

**Stack Overflow's CC-BY-SA problem.** User contributions are CC-BY-SA licensed, with the
version having changed over the site's history (CC-BY-SA 2.5 → 3.0 → 4.0) `[U on exact dates
-- stackoverflow.com is blocked from this environment; verify before citing]`. The consequence
is concrete: **pasting a non-trivial SO snippet into your codebase imports a ShareAlike
obligation and an attribution requirement.** Most codebases have done this and none of them
attribute it. Treat large verbatim SO pastes as a real finding, small idiomatic ones as noise
(short snippets may fall below the copyrightability threshold -- a legal question, not one the
agent should resolve). SO's AI-data licensing deals (OpenAI, Google) are `[U]`.

### 1.6 Font licensing -- the underappreciated trap

**SIL OFL 1.1** (`OFL-1.1`) `[V]` -- **there is no OFL 1.2**; the license is unchanged since
2007, only the FAQ has advanced (to 1.1-update7, Nov 2023) `[V-2026-08]`.

Five conditions `[V]`:
1. *"Neither the Font Software nor any of its individual components... may be sold by
   itself."*
2. may be bundled, redistributed, and sold **with software**, provided the notice and license
   travel with it
3. **Reserved Font Name**: no Modified Version may use the RFN without written permission
4. no using the authors' names to promote a Modified Version
5. *"must be distributed entirely under this license, and must not be distributed under any
   other license"*

**Two FAQ answers that overturn common beliefs** `[V-2026-08]`:
- **Subsetting IS modification.** *"Removing any parts of the font when delivering a webfont to
  a browser, including unused glyphs and smart font code, is considered modification."*
  Consequence: **your subsetted webfont may not use the Reserved Font Name.** Nearly every web
  build pipeline subsets fonts. This is a live, widespread, quiet violation.
- **Nothing is reserved by default.** RFN only applies where actually declared after the
  copyright statement -- a change introduced in OFL 1.1. Check for a declared RFN before
  raising the finding.

Clause 5 is the other sharp edge: an OFL font **cannot** be relicensed under your app's
blanket proprietary terms. It must ship under OFL, with its notice.

**Font Awesome Free** is the canonical multi-license asset trap `[V-2026-08]`, verified from
its LICENSE.txt -- it is **three licenses in one package**:
- **Icons: CC-BY-4.0** (SVG and JS files) -- attribution required
- **Fonts: OFL-1.1**, with **Reserved Font Name "Font Awesome"** -- so a subsetted icon font
  may not keep the name
- **Code: MIT** (all non-font, non-icon files)

GitHub reports the repo as `NOASSERTION` because no single SPDX identifier fits `[V-2026-08]`
-- exactly the case that falls into a scanner's "unknown" bucket.

**Proprietary font EULAs** are where the real money risk is, and they are **not** open-source
licenses: typical restrictions include pageview-capped webfont licensing, per-app embedding
fees, bans on converting formats, and bans on modification (which subsetting may violate).
Specific foundry terms (MyFonts pageview tiers, Adobe Fonts app-embedding rules) are `[U]` --
**do not guess these; read the actual EULA.**

**The practical failure mode:** a designer drops a `.ttf` into the repo with no license file.
That font is `unknown` and must be treated as a blocker until someone produces the license.

**Icon set licenses** `[V-2026-08]`, verified from repo LICENSE files:

| Set | License |
|---|---|
| Material Symbols / Icons | `Apache-2.0` |
| Heroicons, Bootstrap Icons, Feather | `MIT` |
| Lucide | `ISC` **+ MIT** for ~150 Feather-derived icons (listed by name) -- both notices must ship |
| Twemoji | MIT code / CC-BY-4.0 graphics; live at `jdecked/twemoji` |
| Noto Emoji | OFL-1.1 fonts + Apache-2.0 assets |
| Simple Icons | CC0-1.0 -- **but its disclaimer warns CC0 does not license the brands** |

Simple Icons is the trademark trap in miniature: a CC0 SVG of a company logo gives you no right
to use that company's mark.

### 1.7 CLA vs DCO, and why dual-licensing businesses need one

**DCO** (Developer Certificate of Origin) **1.1** `[V]`, at developercertificate.org, © 2004,
2006 The Linux Foundation. Four clauses (a)-(d) certifying you have the right to submit the
contribution `[V]`. Mechanism: a `Signed-off-by: Name <email>` trailer, added by
`git commit -s` `[V]`. The kernel requires a real identity -- *"sorry, no anonymous
contributions"* `[V]`. Git deliberately provides **no config to enable `-s` by default** `[V]`.

A DCO is an **assertion by the contributor**. It grants the project nothing beyond the
inbound license.

**CLA** (Contributor License Agreement) is an **actual grant** to the steward.
- **Apache ICLA/CCLA** `[V-2026-08]`: contributors *"retain full rights to use their original
  contributions for any other purpose"* -- it is a license, **not** an assignment. Note ASF's
  own carve-out: *"small contributions to Apache projects are made under clause 5 of the
  Apache-2.0 license"*; ICLAs are required of committers and large contributors `[V]`. A CCLA
  does **not** remove the need for individual ICLAs `[V]`.
- **Project Harmony** `[V-2026-08]`: templates frozen at **v1.0 since 2011-07-04**, covering
  both license and **assignment** variants. Site up, nothing moving in ~15 years.
- **FSFE Fiduciary Licence Agreement (FLA-2.0)** `[V-2026-08]`: contributor transfers exclusive
  economic rights to a Trustee, who licenses them back. Distinctive anti-abuse clause: *"if the
  Trustee acts against the principles of Free Software, all granted rights and licences return
  to their original owners."* FSFE is **not currently accepting new projects** into the
  Fiduciary Programme. Authors: Matija Šuklje and Catharina Maracke; original by Axel Metzger
  and Georg Greve.
- **Tooling** `[V-2026-08]`: **CLA Assistant** -- hosted service live, **codebase dormant since
  2024-06**. **EasyCLA** -- moved to `linuxfoundation/easycla`, actively developed.

**"inbound=outbound"** -- contributions arrive under the same license the project ships. GitHub
codifies it in **ToS §D.6** `[V]`: *"Whenever you add Content to a repository containing notice
of a license, you license that Content under the same terms... **This is widely accepted as the
norm in the open-source community; it's commonly referred to by the shorthand
'inbound=outbound'.**"* Note the same clause says an explicit CLA **supersedes** this default
`[V]`. Attribution of the coinage to Richard Fontana is `[U]` -- say "associated with," not
"coined by."

**Why dual-licensing businesses need a CLA:** selling a proprietary license to the same code
you ship as GPL requires holding rights broad enough to sublicense. Inbound=outbound alone
gives the steward only the outbound license -- it cannot relicense contributors' code
proprietarily. **A CLA (or assignment) is the mechanism that makes both dual-licensing and
unilateral relicensing possible.** This is the direct causal link between CLAs and the
relicensing wave in §5.

---

## 2. Compatibility -- the actual rules

### 2.1 Compatibility is directional

The core asymmetry: **permissive flows into copyleft; copyleft does not flow back out.**

```
                    (one-way, downhill)
MIT / ISC / BSD-2 ──────────────────────────┐
                                            ▼
Apache-2.0 ────────────────────────────►  GPL-3.0 / AGPL-3.0
    │  (NOT into GPL-2.0-only)               ▲
    │                                        │
    └──────────────────────────────►  LGPL-3.0
                                            ▲
MPL-2.0 (unless Exhibit B) ─────────────────┘
                                     
GPL-2.0-only ──✗── GPL-3.0     (no path: "only" forbids upgrade)
GPL-2.0-or-later ──✓── GPL-3.0
```

"Compatible" means: **can these be combined into one distributed work, and under what license
does the result ship?** The result ships under the *more restrictive* license. Apache-2.0 code
placed in a GPL-3.0 project ships as GPL-3.0 -- the Apache code's own terms (including its
NOTICE obligation) still ride along.

### 2.2 Apache-2.0 and GPL-2.0 -- the specific reason

**Apache-2.0 is compatible with GPL-3.0 but NOT with GPL-2.0-only.** `[V]`

The Apache Software Foundation states the reason directly: the FSF treats *"the patent
termination and indemnification provisions as restrictions not present in the older GPL
license"* `[V]`. The mechanism is **GPL-2.0 §7** `[V]`: if conditions are imposed on you that
contradict the License, *"you may not distribute the Program at all."* Apache-2.0 §3's
patent-litigation termination is such an additional condition. GPL-3.0 §11 was written to
accommodate patent terms, which is what resolves it there.

ASF's own position: *"you should always try to obey the constraints expressed by the copyright
holder when redistributing their work"* -- they defer to the FSF's reading even where they
disagree `[V]`.

**Why this bites in practice:** the Linux kernel is **GPL-2.0-only**. Every "can we pull this
Apache-2.0 library into a kernel module?" question hits this wall. It also catches Android
projects mixing Apache-2.0 userspace with GPL-2.0 kernel code -- fine as separate programs,
not fine combined into one work.

### 2.3 Derivative work, combined work, and mere aggregation

**These are legal terms, not technical ones, and the agent must not resolve them.** What it
can do is describe where the line is contested.

- **Mere aggregation** `[V]`: GPL-2.0 §2 -- separate works on the same distribution medium do
  not infect each other. A Linux distro ISO, a `node_modules` tree, a Docker image containing
  independent programs: aggregation, not combination.
- **Combined work**: linked into a single program image. Clearly in scope for copyleft.
- **The contested middle**: dynamic linking, plugins, IPC.

**The FSF's position**, verbatim from the GPL FAQ (`#GPLStaticVsDynamic`) `[V-2026-08, via
web.archive.org mirror -- gnu.org origin was intermittently unreachable]`:

> **No.** Linking a GPL covered work statically or dynamically with other modules is making a
> combined work based on the GPL covered work. Thus, the terms and conditions of the GNU General
> Public License cover the whole combination.

The "No" answers "does the GPL impose *different* requirements for static vs dynamic linking" --
**the FSF treats them identically.** So the folk rule "dynamic linking is safe" is not the FSF's
position at all.

**The FSF's actual boundary test** (`#MereAggregation`), verbatim:

> a proper criterion depends both on the mechanism of communication (exec, pipes, rpc, function
> calls within a shared address space, etc.) and the semantics of the communication (what kinds
> of information are interchanged).
>
> If the modules are included in the same executable file, they are definitely combined in one
> program. If modules are designed to run linked together in a shared address space, that almost
> surely means combining them into one program.
>
> By contrast, pipes, sockets and command-line arguments are communication mechanisms normally
> used between two separate programs. So when they are used for communication, the modules
> normally are separate programs. But if the semantics of the communication are intimate enough,
> exchanging complex internal data structures, that too could be a basis to consider the two
> parts as combined into a larger program.

**Two things follow.** Pipes, sockets, and CLI arguments are only *presumptively* arms-length,
and the presumption is rebuttable by intimate data exchange. And **containers change nothing**
(`#AggregateContainers`).

**Plugins** (`#GPLPlugins`) `[V-2026-08]`: fork/exec with "intimate communication by sharing
complex data structures" makes one combined program; a simple fork/exec that does not establish
intimate communication leaves them separate. Dynamic linking with mutual function calls and
shared data structures is "a single combined program." Invoking only the plugin's `main` and
awaiting return is *"a borderline case."* Shared memory with complex data structures is
*"pretty much equivalent to dynamic linking."*

**GPL applied to program output** (`#GPLOutput`) `[V-2026-08]`: *"The output of a program is not,
in general, covered by the copyright on the code of the program."* And: *"when a program
translates its input into some other form, the copyright status of the output inherits that of
the input it was generated from."* The exception is output that copies substantial program
content -- a screen of text or art from the program, or game audio. **This is the general rule
behind §2.3's code-generation analysis**, and the Bison skeleton is its classic exception.

**System library exception -- the FAQ closes a loophole engineers reach for**: libraries qualify
as System Libraries *"only as long as they're not distributed with the program itself. If you
distribute the DLLs with the program, they won't be eligible for this exception anymore."*
`[V-2026-08]`

**LGPL static vs dynamic** (`#LGPLStaticVsDynamic`) `[V-2026-08]`: static linking requires
providing *"your application in an object (not necessarily source) format, so that a user has
the opportunity to modify the library and relink."* Dynamic linking against a library already on
the user's machine carries no source obligation -- but if you convey the LGPL library yourself,
statically or dynamically, you must convey its source.

**[DISPUTED]** Many practitioners and some counsel read this more narrowly, arguing derivation
is a question of copyright law (substantial similarity, incorporation of expression) that the
license author cannot settle by assertion. **No US court has squarely decided the linking
question.** The agent should present both readings and route to counsel -- it must never assert
that dynamic linking is or is not derivation.

**Arms-length boundaries that are widely (not universally) accepted as separating works:**
- **Separate processes communicating over a socket or pipe**, with the boundary not being an
  intimate data-structure-sharing interface
- **Command-line invocation** of a separate executable
- **The Linux syscall boundary** -- explicitly carved out by the kernel's own note `[V]`:
  *"This copyright does not cover user programs that use kernel services by normal system
  calls - this is merely considered normal use of the kernel, and does not fall under the
  heading of 'derived work'."* This exists **because** the syscall boundary would otherwise be
  arguable.

**The system library exception** `[V]`: GPL-3.0 §1 excludes "System Libraries" -- components
that come with a Major Component (OS, compiler, kernel) and are not part of the work -- from
Corresponding Source. GPL-2.0 §3 has a narrower analogue for things *"normally distributed with
the major components of the operating system."* This is what lets a GPL program link the
platform libc. It does **not** cover libraries you chose to bundle.

**Containers**: a container image is closer to aggregation than combination -- but a container
whose entrypoint statically links GPL code into your binary is combination, and the container
boundary changes nothing. **Distributing a container image is distribution**, which triggers
source-offer obligations.

**Code generation**: does output inherit the generator's license? **It depends on whether the
generator copies its own code into the output.** The existence of the **Bison exception**
proves the point `[V]`: Bison's skeleton is copied into generated parsers, so without the
exception the output would be GPL. The exception grants permission *"to create a larger work
that contains part or all of the Bison parser skeleton and distribute that work under terms of
your choice."* A generator that emits only your own grammar's logic, copying nothing, produces
output the generator's license does not reach. **Check whether the generator embeds a runtime
or skeleton.**

**Header-only libraries**: the whole library is compiled into your binary, so a copyleft
header-only library is combination, not linkage. LGPL's ten-line thresholds (§5 in 2.1, §3 in
3.0) `[V]` are what carve out trivial inline functions and macros -- above that, the object
file is a derivative.

### 2.4 Static linking in Rust and Go -- why LGPL is genuinely hard

**Rust and Go link statically by default.** A Cargo build produces one binary with every crate
compiled in; Go produces a single static binary. There is no shared-library mechanism in the
ordinary workflow.

This means **LGPL-3.0 §4(d)(1)** -- the shared-library escape hatch -- is simply unavailable
`[V]`. You are forced onto **§4(d)(0)**: ship the Minimal Corresponding Source **and** your
application's code *"in a form suitable for, and under terms that permit, the user to
recombine or relink"* `[V]`.

For a proprietary application, that means **shipping your object files** so a user can relink
against a modified library. Most companies will not do this. The result: **LGPL is
substantially harder in Rust/Go than in C/C++**, and in practice teams either avoid LGPL crates
or accept an obligation they have not actually satisfied.

Rust ecosystem specifics `[V-2026-08]`: the Rust toolchain and stdlib are `Apache-2.0 OR MIT`;
the project itself uses **REUSE** for compliance and ships a `COPYRIGHT.html` with each binary
release -- a good model for attribution done right.

**MPL-2.0 is the pragmatic middle** in these ecosystems: file-level copyleft with no linking
distinction `[V]`, so static linking raises no relinking obligation.

### 2.5 The GPL exception ecosystem

Exceptions are **additional permissions** (GPL-3.0 §7) that narrow copyleft for a specific
combination. SPDX expresses them with `WITH`: `GPL-2.0-only WITH Classpath-exception-2.0`.

- **Classpath exception** (`Classpath-exception-2.0`) `[V]`: *"the copyright holders of this
  library give you permission to link this library with independent modules to produce an
  executable, regardless of the license terms of these independent modules, and to copy and
  distribute the resulting executable under terms of your choice."* This is why **OpenJDK is
  usable in proprietary applications** -- GPL-2.0 alone would make every Java program a
  derivative of the class library.
- **GCC Runtime Library Exception** (`GCC-exception-3.1`) `[V]`: permits propagating Target Code
  that combines the Runtime Library with Independent Modules, conditioned on an *"Eligible
  Compilation Process"* -- one done *"using GCC, alone or with other GPL-compatible software,
  or... without using any work based on GCC."* **This is why compiling with GCC does not make
  your binary GPL** -- despite libgcc being linked into it.
- **Bison exception** (`Bison-exception-2.2`) `[V]`: see §2.3.
- **Linux syscall note** (`Linux-syscall-note`) `[V]`: see §2.3.

**Finding pattern:** an exception that is *stripped* in vendoring, or a dependency recorded as
`GPL-2.0-only` when it is actually `GPL-2.0-only WITH Classpath-exception-2.0`, produces a
false blocker. Conversely, recording the exception when the vendored copy dropped it produces a
false clear. **The `WITH` clause is load-bearing metadata.**

### 2.6 Relicensing and who can do it

**Only the copyright holders can relicense.** Consequences:

- **With a CLA or assignment** granting the steward sublicensing rights, the steward can
  relicense unilaterally. This is the mechanism behind every vendor relicensing in §5.
- **Without one**, relicensing requires **every contributor's consent** -- which for a
  long-lived project is effectively impossible. Hence: rewriting holdouts' code (VLC did
  exactly this `[V]`), or abandoning the change.
- **"or later" clauses** provide a limited unilateral path: `GPL-2.0-or-later` code can be
  distributed under GPL-3.0 without asking anyone.
- **Permissive-to-anything** always works: MIT code can be relicensed into a proprietary
  product, because MIT permits it. The original MIT grant survives for the original code.

**VLC's forward guard** is the instructive engineering fix `[V]`: after the App Store episode,
any commit implicitly allows relicensing to any OSI-approved license -- designing away the
holdout veto at contribution time.

---

## 3. Obligations that produce shipping requirements

This is the section that turns licensing into engineering work. **Every obligation below has a
build-system or UI deliverable.**

### 3.1 Attribution and notice

| License | Source distribution | Binary distribution |
|---|---|---|
| MIT | notice + license text | **same** -- "all copies or substantial portions" `[V]` |
| BSD-2/3 | notice (cl. 1) | notice *"in the documentation and/or other materials"* (cl. 2) `[V]` |
| Apache-2.0 | License copy, modified-file notices, retained notices | **plus NOTICE contents** (§4d) `[V]` |
| ISC | notice | notice |
| Zlib | notice must not be removed | **no binary requirement** (cl. 3 is source-only) `[V]` |
| OFL-1.1 | notice + license, must stay entirely under OFL | same `[V]` |
| CC-BY-4.0 | full attribution incl. modification indication | same `[V]` |

**"Include the license text" is the requirement people ignore.** Listing "MIT" in a
dependency table is not compliance. The **full text**, with the **original copyright line**
(each dependency's own copyright holders, not yours), must ship.

**Apache-2.0 NOTICE propagation** `[V]` deserves its own callout because it is the most-missed
permissive obligation:
- It is a **separate file** from LICENSE
- Its contents must be **carried into your derivative work's NOTICE**, or into documentation,
  or into a display generated by the work
- It **propagates transitively** -- a NOTICE three levels down in the dependency graph still
  binds
- Generic license-aggregation tools frequently **collect LICENSE files and skip NOTICE files**

**Finding:** any Apache-2.0 dependency shipping a NOTICE file, where the build produces no
aggregated NOTICE, is a **major** finding. Attribution in an About box does not cure it unless
the NOTICE *contents* are what appear there.

### 3.2 Source offer under GPL

If you **distribute** (convey) GPL binaries, you owe corresponding source. Three routes under
GPL-2.0 §3 `[V]`: ship source alongside; **a written offer valid for at least three years**; or
pass along the offer you received (noncommercial only).

**"Corresponding source" is broader than the application code** `[V]`: *"all the source code
for all modules it contains, plus any associated interface definition files, plus the scripts
used to control compilation and installation of the executable."*

Engineering consequences:
- **Build scripts, Makefiles, and configuration are in scope.** A tarball of `.c` files is not
  compliance if it cannot be built.
- **Toolchain information** matters -- the recipient must be able to actually produce a working
  binary.
- The **three-year clock** is a records-retention obligation. Someone must keep the exact source
  for the exact shipped version for three years, indexed by version. This is a real
  infrastructure requirement, and it is why an offer is usually worse operationally than just
  shipping source.
- **GPL-3.0 §6 Installation Information** for User Products: the *"methods, procedures,
  authorization keys, or other information required to install and execute modified versions"*
  `[V]`. On a locked bootloader or a signed firmware image, this is the requirement that
  cannot be met without changing the product's security architecture.

### 3.3 AGPL network trigger

Covered in §1.3. The engineering deliverables when §13 applies `[V]`:
- A **prominent offer** to *all users interacting remotely* -- typically a link in the UI or an
  API response header, not a buried footer
- **Corresponding Source of your version**, served **from a network server at no charge**
- The offer must use *"some standard or customary means of facilitating copying"*

**"We only use it internally" is not automatically safe** -- §13 says *all users interacting
through a computer network*.

### 3.4 LGPL relinking -- often the fatal one

Covered in §1.2 and §2.4. Restated because it is the obligation most often discovered too late:

**For a statically-linked mobile binary, LGPL is usually not satisfiable in practice.** You
would need to ship object files sufficient for a user to relink your proprietary application
against a modified library -- and on iOS, the relinked binary could not be signed and installed
anyway. **LGPL-3.0 §4(e)** compounds this by importing GPL-3.0 §6 Installation Information,
including authorization keys the developer does not possess `[V]`.

**Finding: an LGPL dependency statically linked into a signed mobile binary is a blocker.**

### 3.5 App store conflicts

**The mechanism** `[V]`: GPL-2.0 §6 / GPL-3.0 §10 forbid imposing further restrictions on
recipients. Apple's terms impose them.

Verified from Apple's current terms `[V-2026-08]`: use limited to devices you own or control,
limits on simultaneous Apple Account and device associations, *"you may not distribute or make
the Licensed Application available over a network where it could be used by multiple devices at
the same time"*, *"You may not transfer, redistribute or sublicense"*, and *"You may not tamper
with or circumvent any security technology."*

**The history** `[V-2026-08]`:
- **GNU Go, 2010**: FSF notified Apple 2010-05-25; removed by 2010-05-26. FSF did **not** sue,
  and stressed both Apple *and* the uploading developers were violators.
- **VLC, 2010-2013**: Rémi Denis-Courmont complained 2010-10-26 -- **as an individual copyright
  holder, against his own project's wishes**; VideoLAN's Jean-Baptiste Kempf publicly opposed
  him. Engine relicensed to LGPLv2.1+ (completed 2011-12-21), modules 2012-11-20, back on the
  store 2013-07-18. One developer refused consent and **their code was rewritten.**
  **Correction to the common narrative:** the app is **bi-licensed GPLv2+ and MPLv2**, with
  MPLv2 the branch used for App Store distribution -- not LGPL alone, and not MPL-2.0 alone.
  VideoLAN's own FAQ declined to claim the relicensing had solved the problem.

**Two nuances that sharpen the finding** `[V-2026-08]`:
1. **§10, not §6, is the operative conflict.** GPL-3.0 §6's Installation Information triggers
   when conveyance *"occurs as part of a transaction in which the right of possession and use
   of the User Product is transferred"*. An app-store download is arguably a separate
   transaction from the device purchase. §6 **does** bite for preinstalled software.
2. **Apple's terms have drifted.** The current EULA drops the numeric device cap and adds an
   explicit open-source carve-out to its no-modification clause. The surviving conflict is the
   **no-redistribution / no-transfer** clause against GPL §6 rights. The FSF appears not to
   have published an updated analysis, so **"still a violation today" is genuinely unsettled**
   -- present it as a counsel question, not a settled fact.

**Google Play is materially different** `[V-2026-08]`. The Developer Distribution Agreement
states *"Users are allowed unlimited reinstalls"*; there is no device cap and no usage-rules
construct binding the end user. **Decisive evidence: VLC for Android ships on Play as GPLv2+
with no relicensing** -- same project, same holdout contributors, different store terms.

**Practical rule for review:** GPL-family (not LGPL) code in an iOS app is a **blocker**
pending counsel. The same code on Google Play is a **major** finding worth review but not
structurally impossible.

### 3.6 Per-platform attribution surfaces

The deliverable that satisfies §3.1. Verified tooling status `[V-2026-08]`:

**iOS / Apple**
- **Settings bundle acknowledgements** (`Settings.bundle`, `Acknowledgements.plist`) -- the
  platform-idiomatic surface
- **CocoaPods** generates `Pods-<Target>-acknowledgements.plist`. **CocoaPods entered
  maintenance mode 2024-08-13** (security fixes and ~2 Xcode-compat releases per year; Specs
  repo eventually read-only). Do not build new long-term pipelines on it.
- **SwiftPM built-in license aggregation**: no evidence found `[U]` -- treat as absent and
  generate notices yourself.

**Android**
- `com.google.android.gms:play-services-oss-licenses` (**17.5.1**) + `oss-licenses-plugin`
  (**0.13.0**), both active. The activity is now
  `com.google.android.gms.oss.licenses.**v2**.OssLicensesMenuActivity` -- older snippets omit
  `.v2.`
- **AboutLibraries** (mikepenz), **15.1.1**, active
- **jaredsburrows/gradle-license-plugin** and **cashapp/licensee**, both active

**Web** -- a `/licenses` page or `THIRD-PARTY-NOTICES.txt`
- `rollup-plugin-license` **3.7.1**, active
- `license-webpack-plugin` **dormant since 2022-02**

**Rust** -- `cargo-about` (**0.9.2**, active) renders a templated notices file
**Go** -- `go-licenses save` collects license files
**Desktop/Electron** -- `app.setAboutPanelOptions`; `credits` is macOS/Windows only, and macOS
auto-loads `Credits.html`/`.rtf` from the bundle

**Reviewer's question:** *"Where does a user of the shipped artifact actually read these
notices?"* If there is no answer, that is a **major** finding regardless of what the repo
contains.

### 3.7 Trademark

**Licenses grant copyright and patent -- not trademark.** Apache-2.0 §6 is explicit `[V]`:
no permission to use *"the trade names, trademarks, service marks, or product names of the
Licensor, except as required for reasonable and customary use in describing the origin of the
Work and reproducing the content of the NOTICE file."*

Practical consequences:
- **Forking requires renaming.** Every major fork is also a rename: OpenTofu (not Terraform),
  Valkey (not Redis), OpenSearch (not Elasticsearch), OpenVox (not Puppet), Iceweasel (not
  Firefox, historically).
- **Elastic v. Amazon settled 2022-02-16** `[V-2026-08]` -- the cleanest teaching case that
  **trademark, not copyright, forces renames.**
- **Rust's current policy expressly permits** naming crates and `cargo-foobar` subcommands
  `[V-2026-08]` -- a reversal of the 2023 draft's most-criticized restriction.
- **WordPress** `[V-2026-08]`: the WordPress Foundation owns the marks and Automattic holds
  *"the exclusive license"*; the policy text itself addresses the WP Engine confusion and
  directs hosts to say "Hosting for WordPress." Litigation outcome `[U]`.
- A permissively-licensed logo file is **still a trademark**. Simple Icons says so explicitly
  `[V-2026-08]`.

---

## 4. Practical compliance engineering

### 4.1 SBOM: SPDX vs CycloneDX

`[V-2026-08]`

| | Current version | Date | Standards status |
|---|---|---|---|
| **SPDX** | **3.0.1** stable; 3.1-RC1 (2026-01-24) | 2024-12-17 | **ISO/IEC 5962:2021 covers SPDX 2.2.1**, not 3.x |
| **CycloneDX** | **1.7** (1.7.1 patch 2026-06-02) | 2025-10-21 | **ECMA-424, 2nd ed., Dec 2025** |
| **SPDX License List** | **3.28.0** | **2026-02-20** | -- |

**The structural shift most people missed** `[V-2026-08]`: the BOM stack moved into **Ecma
TC54** in Dec 2025 -- **ECMA-424** (CycloneDX 1.7), **ECMA-427** (Package-URL / purl),
**ECMA-428** (Common Lifecycle Enumeration). purl becoming a formal standard matters because
tooling (including GitHub's `allow-dependencies-licenses`) consumes purl identifiers.

**Retire the "SPDX = legal, CycloneDX = security" split.** It was conventional wisdom and it
is now dated: SPDX 3.0 added a Security profile; CycloneDX 1.7 added patent/IP transparency.
Both have grown into each other's territory `[V-2026-08]`. State the actual capability, not the
folk taxonomy.

SPDX 3.x organizes into **profiles** (Core, Software, Licensing, Security, Build, AI, Dataset,
Hardware, Lite, Services, Supply Chain) `[V-2026-08]`. The **AI and Dataset profiles** are
directly relevant to model licensing.

**SPDX License List cadence is irregular** -- 2-3 releases a year, and only **one** in all of
2025 (a 7-month gap from 3.27.0 to 3.28.0) `[V-2026-08]`. Do not assume quarterly.

**Regulatory drivers**
- **US EO 14028** (2021) and CISA SBOM minimum elements. **EO 14306 (June 2025) amends
  EO 14144, not EO 14028**, and never contains the string "software bill of materials" -- **do
  not claim SBOM requirements were revoked** `[V-2026-08]`. CISA's 2025 minimum-elements
  refresh appears to still be **draft** `[U -- cisa.gov blocks automated fetch]`.
- **EU Cyber Resilience Act -- Regulation (EU) 2024/2847** `[V-2026-08]`. Published 2024-11-20;
  in force 2024-12-10. **Article 14 reporting obligations apply from 2026-09-11**; Chapter IV
  from 2026-06-11; **main obligations from 2027-12-11**.
  **The CRA does not require giving customers an SBOM.** Annex II(9): *"**If the manufacturer
  decides** to make available the software bill of materials to the user..."* Mandatory
  disclosure runs **only to a market surveillance authority on reasoned request** (Annex
  VII(8)). Annex I Pt II(1) requires an SBOM *"in a commonly used and machine-readable format
  covering **at the very least the top-level dependencies**"* -- a floor, not a ceiling. **This
  is the most commonly misstated fact in the space.**
  The CRA also creates the **"open-source software steward"** actor, and fines do not apply to
  non-commercial open-source developers.

### 4.2 The seam with the security agent

**SBOM serves both lenses; the artifact is shared, the questions differ.**

| | This agent asks | The `security` agent asks |
|---|---|---|
| SBOM | What license is each component under? Are obligations satisfiable? | What known vulnerabilities are in these versions? Is the component authentic? |
| Dependency added | Is the license allowed? Does it change our shipping obligations? | Typosquat? Malicious postinstall? Maintainer compromise? |
| Version bump | **Did the license change?** | Did a CVE get fixed or introduced? |
| Tooling | Syft generates the SBOM; **this agent reads the license fields** | Grype/Trivy/Dependency-Track consume it for vulns |

**Route to `security`:** typosquatting, malicious packages, dependency confusion, provenance,
SLSA, signing, CVE triage.
**Own here:** license identification, compatibility, obligations, attribution, policy gates.
**Shared:** SBOM generation and completeness -- an incomplete SBOM fails both lenses, and
that finding belongs to whichever agent notices it. **OpenChain reflects the same split**:
**ISO/IEC 5230** (license compliance, re-confirmed current 2026-07-30) and **ISO/IEC 18974**
(security assurance, :2023 -- note the year) `[V-2026-08]`.

### 4.3 SPDX identifiers as vocabulary

`SPDX-License-Identifier:` headers are the standard machine-readable declaration `[V]`:

```
// SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: GPL-3.0-only WITH Classpath-exception-2.0
// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-License-Identifier: Apache-2.0 AND (MIT OR GPL-2.0-only)
```

Expression syntax `[V-2026-08]`: precedence is `+` > `WITH` > `AND` > `OR`; **no whitespace
before `+`**; operators are case-sensitive (all-upper or all-lower); identifiers are
case-insensitive. `LicenseRef-` marks a license not on the SPDX list; SPDX 3.0 adds
`AdditionRef-`.

**`OR` means the recipient chooses** (Rust's `Apache-2.0 OR MIT` -- you pick). **`AND` means
both apply simultaneously** -- all obligations stack. Confusing these inverts the analysis.

**Deprecated identifiers** `[V]`: bare `GPL-2.0`, `GPL-3.0`, `LGPL-2.1`, `AGPL-3.0` etc. were
replaced in License List v3.0 by `-only` / `-or-later` forms. Flag the bare forms as
under-specified (see §1.3).

**REUSE Specification** (FSFE) `[V-2026-08]`: **3.3** (2024-11-14) is current; **3.2**
introduced **`REUSE.toml`**, deprecating `.reuse/dep5` (which remains supported until at least
2029). Three rules: put license texts in `LICENSES/`, add `SPDX-FileCopyrightText` and
`SPDX-License-Identifier` headers to every file, and verify with the `reuse` tool
(**v6.2.0**, 2025-10-27; development moved to **Codeberg**, GitHub is a mirror).

### 4.4 Scanning and tooling -- current state

`[V-2026-08]`, verified against repository metadata and release pages.

**Maintained and practical defaults**

| Tool | Status | Use |
|---|---|---|
| **ORT** (OSS Review Toolkit) | 92.4.0, ships weekly | The serious open-source end-to-end: analyze → scan → evaluate policy → report |
| **ScanCode Toolkit** | v32.5.0 | Deep file-level license detection. **org moved `nexB` → `aboutcode-org`** |
| **Syft** | v1.51.0 | SBOM generation (SPDX + CycloneDX) |
| **Dependency-Track** (OWASP) | 5.0.5 | SBOM ingestion and continuous monitoring |
| **cargo-deny** | 0.20.2 | Rust policy gate |
| **cargo-about** | 0.9.2 | Rust attribution file generation |
| **FOSSology** (Linux Foundation) | 4.7.1 | License scanning workbench |
| **licensee** (GitHub) | v10.1.0 | License identification from a file |
| **go-licenses** (Google) | v2.0.1 (2025-09-08) | **Not archived**, but ~11.5 months of default-branch silence |
| **Trivy**, **Grype**, **osv-scanner** | active | Primarily security; license fields secondary |

**Commercial**: FOSSA, Snyk (license side), **Black Duck** (**spun out of Synopsys in
October 2024**, not 2025), Mend (formerly WhiteSource). Specific format-support claims are
`[U]` -- vendors restructured their doc trees.

**Dead or dormant -- flag if you see them in a pipeline**
- **`davglass/license-checker`**: last commit **2019-01-10**, no GitHub releases. Use the
  **`license-checker-rseidelsohn`** fork (v5.0.1).
- **`pivotal/LicenseFinder`**: last commit **2024-05-08**.
- **`license-webpack-plugin`**: dormant since 2022-02.
- **`uv` has no license reporting** (issue #8156 open). Do not recommend it for this.
- **`microsoft/sbom-tool`**: slowing; still advertises SPDX 2.2.

**GitHub-native** `[V-2026-08]`
- SBOM export (`GET /repos/{owner}/{repo}/dependency-graph/sbom`) emits **SPDX-2.3**, not 3.x
- **`dependency-review-action` v5.0.0** (2026-05-08, node24 -- needs runner ≥2.327.1):
  **`deny-licenses` is deprecated; use `allow-licenses`**

**Two silent-failure traps in `dependency-review-action`** `[V-2026-08]`:
1. License data comes from **ClearlyDefined**, not GitHub's own detection -- so results can
   disagree with what the repo page shows.
2. **Unlicensed dependencies produce only an informational notice. They do not fail the
   action.** A pipeline that "checks licenses" this way passes unlicensed code silently.

### 4.5 Policy as code

**cargo-deny `deny.toml`** `[V-2026-08]` -- the v2 schema is **not** what most blog posts show.

Removed in **0.16.0 (2024-08-02)**, now a **hard error**: `unlicensed`, `deny`, `copyleft`,
`allow-osi-fsf-free`, `default`.

The current model is **deny-by-default**:

```toml
[licenses]
# Anything not listed is denied. There is no separate "deny" list any more.
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-3-Clause", "ISC"]

# Per-crate escape hatches -- each one is a decision someone made on the record.
exceptions = [
    { allow = ["MPL-2.0"], crate = "webpki-roots" },
]

# askalono detection confidence. Below this, the license is "unknown".
confidence-threshold = 0.8

# Crates with no machine-readable license metadata: assert one, pinned to a file hash.
[[licenses.clarify]]
crate = "some-crate"
expression = "MIT"
license-files = [{ path = "LICENSE", hash = 0xbd0eed23 }]

[licenses.private]
ignore = true  # skip unpublished workspace crates
```

**Because the model is deny-by-default, an unlicensed crate fails automatically** -- the old
advice to "also deny unknown" describes a schema that no longer exists. **The correct modern
finding is the inverse: check that `confidence-threshold` has not been lowered and that
`clarify` entries are not being used to wave through crates nobody actually reviewed.**

**Allowlist vs denylist.** Allowlists are correct. A denylist fails open: every license nobody
thought of passes, which is precisely the set you most need to look at.

**The "unknown" bucket is the real risk.** In every ecosystem, the dangerous finding is not
"GPL detected" -- that is loud and someone will act on it. It is:
- no license file at all
- a license file the scanner cannot match above threshold
- `NOASSERTION` (what GitHub reports for Font Awesome and Lucide `[V-2026-08]`)
- a custom or modified license (`LicenseRef-*`)
- a multi-license package where one identifier does not fit

**CI must fail on unknown.** A pipeline that only fails on a denylist of known-bad licenses
provides approximately no protection.

### 4.6 License changes between versions

**A package can relicense in a patch bump.** Lockfile diffs show version changes, not license
changes -- so this is invisible to normal review.

Canonical cases, all verified `[V-2026-08]` (details in §5):
MongoDB (AGPL→SSPL, 2018-10-16) · Elastic (Apache-2.0→SSPL+ELv2 in 7.11, 2021-01-14; **+AGPL
2024-08-29**) · HashiCorp (MPL-2.0→BUSL-1.1, 2023-08-10) · Redis (BSD-3→RSALv2+SSPL in 7.4,
2024-03-20; **+AGPLv3 in Redis 8, 2025-05-01**) · Sentry (BSL 2019-11-06 → FSL 2023-11-17) ·
Grafana (Apache-2.0→AGPL-3.0, 2021-04) · CockroachDB (BUSL → proprietary CockroachDB Software
License, v24.3.0, 2024-10-01) · Akka (Apache-2.0→BSL, 2022-09-07).

**Engineering requirement: gate on license change, not just version change.** Store the
resolved license expression per dependency per version and diff it in CI. Tools that fail
builds only on a *new* disallowed dependency will miss an existing dependency that relicensed
under you.

**The pin-and-fork consequence:** relicensing is **not retroactive**. Redis 7.2 and earlier
remain BSD-3-Clause `[V]`; Elasticsearch ≤7.10.2 remains Apache-2.0 `[V]`. Pinning to the last
permissive version is a legitimate (if unmaintainable) strategy, and it is exactly what the
forks did.

### 4.7 Transitive dependencies

**The direct-dependency list is not the answer.** A permissive direct dependency can pull a
copyleft transitive one. The obligation attaches to what you *ship*, and you ship the whole
tree.

- Resolve the **full transitive graph**, including optional and feature-gated dependencies that
  are actually enabled in the shipped build
- **Build-time-only dependencies** (test frameworks, linters, codegen tools) generally do not
  ship, so their licenses usually do not create distribution obligations -- **but a codegen
  tool that embeds a runtime or skeleton in its output does** (§2.3)
- **Platform-specific dependencies** differ per target: an iOS build and an Android build have
  different graphs and different obligations
- **Vendored and bundled code** may not appear in the manifest at all

### 4.8 Vendored code, snippets, and AI-generated code

**Vendored code** -- copied into your tree, invisible to the package manager. `git subtree`,
`vendor/`, "we just copied that one file." Its license still applies. **ScanCode or ORT scanning
the actual file tree is the only reliable detection**, because manifest-based tools see nothing.

**Copy-pasted snippets** -- Stack Overflow's CC-BY-SA problem (§1.5), plus blog posts, gists,
and other repos.

**AI-generated code** -- genuinely unresolved, and the agent must say so rather than guess.

What is **verified**:
- **Human authorship is required for US copyright.** *Thaler v. Perlmutter*: D.C. Circuit
  affirmed 2025-03-18; rehearing denied 2025-05-12; **SCOTUS denied certiorari 2026-03-02**
  `[V-2026-08]`. This is now settled at the appellate level.
- **US Copyright Office reports**: Part 1 (Digital Replicas) 2024-07-31; **Part 2
  (Copyrightability) 2025-01-29** -- note **January**, not February; **Part 3 (Generative AI
  Training) remains a pre-publication version released 2025-05-09**, still not final 15+ months
  later `[V-2026-08]`.
- **Whether training on GPL code creates a derivative work is UNDECIDED.** No court has held
  that model weights or model output are derivative works of training inputs `[V-2026-08]`.
  Anyone asserting otherwise, in either direction, is ahead of the law.

The **practical consequence**, flagged as analysis rather than holding: a copyright license
operates by granting permission under a copyright you hold. If no copyright subsists in purely
machine-generated code, there is less to license -- which weakens **copyleft enforcement**
specifically, since GPL's mechanism *is* copyright. Real code is usually mixed (human
prompting, selection, arrangement, editing), so the practical question is which parts are
protectable, not whether any are.

**Vendor IP indemnities** `[V-2026-08]` -- verified from vendor legal terms, and **one very
widely-repeated fact is now wrong**:

> **GitHub Copilot no longer requires the duplicate-detection / public-code filter for
> indemnity coverage.** Microsoft's Customer Copyright Commitment page states: *"as of April 3,
> 2026, there are no additional required mitigations. Use of the Duplicate Detection filter
> feature is no longer required for CCC coverage."* Nearly every guide online still says
> otherwise. Also note GitHub's Copilot Product Specific Terms were **replaced 2026-03-05** by
> the GitHub Generative AI Services Terms.

- **AWS** still **does** condition its §50.10 indemnity on not interfering with or failing to
  enable available filters `[V-2026-08]`. The conditions differ by vendor -- do not generalize.
- **Google Cloud** indemnifies on **two prongs**: generated output *and* training data, with
  exclusions including disregarding source citations or filters `[V-2026-08]`.
- **Anthropic** commercial terms §K.1 cover paid use *including data used to train the model*
  and outputs `[V-2026-08]`.
- **OpenAI** Business Terms §13.1 indemnify against claims that *"the Services"* infringe --
  note it says Services, not Output `[V-2026-08]`. "Copyright Shield" does not appear as a
  current term `[U]`.

**Copilot's public-code filter** still exists and still works as described `[V-2026-08]`: it
checks suggestions *"with their surrounding code of about 150 characters"* against public code
on GitHub and suppresses matches. Org/enterprise settings **override** personal ones. Its
default state is `[U]`. It is now a **code-hygiene** control rather than an indemnity
precondition.

### 4.9 Contributor provenance

Covered in §1.7. In CI terms: **DCO check** (verify `Signed-off-by` matches the commit author)
or **CLA bot** (EasyCLA, CLA Assistant) blocking merge until signed. Pick one deliberately --
the choice determines whether the project can ever relicense (§2.6).

---

## 5. Canonical incidents, cases, and sources

Case law and the people who shape this field live in `~/.claude/rules/licensing-cases.md`, kept separate so this file stays loadable. **Read it when a finding turns on enforceability, remedies, or jurisdiction** -- the four US litigation lines converge on treating these licenses as enforceable contracts because the right to receive source is an "extra element" copyright does not provide, while France reached infringement instead. Remedies differ enormously, which is why jurisdiction is a real question rather than a footnote.

## 6. Schools of thought -- genuine disagreements

**The agent must represent these as contested, not resolve them.** Where a finding depends on
which side is right, say so and route to counsel or to the org's policy owner.

**Copyleft as freedom-preserving vs adoption-limiting.** The FSF/SFC position (Stallman, Kuhn,
Moglen) is that copyleft is the only mechanism that keeps software free for downstream users,
and that permissive licensing donates labor to proprietary vendors. The counter-position -- held
by much of industry and by the Apache and BSD communities -- is that copyleft's compliance
burden suppresses adoption, and that adoption *is* the point. **Both are empirically defensible
and neither is a technical question.** Note the internal irony: **Bradley Kuhn invented the
AGPL's network clause** `[V-2026-08]`, and AGPL is now the license most commonly banned by
corporate policy.

**[DISPUTED] The FSF's linking position vs the practitioner reading.** See §2.3. The FSF says
dynamic linking creates a derivative work. Many practitioners and some counsel say derivation
is a copyright-law question that a license author cannot settle by assertion. **No US court has
squarely decided it.** The agent must present both and never adjudicate.

**Are source-available licenses legitimate, or "open washing"?** Vendors argue that
hyperscalers free-riding on their labor is an existential threat that permissive licensing
cannot answer, and that BUSL/FSL time-limited restrictions are an honest compromise (FSL's
two-year window is deliberately shorter than BUSL's four). Critics -- the OSI, much of the
community -- argue that "source available" marketed as "open" destroys the term's meaning, and
that the definitional confusion is itself the harm. The academic critique is real and citable:
**Liesenfeld & Dingemanse**, "Rethinking open source generative AI: Open washing and the EU AI
Act" (ACM FAccT 2024) `[V-2026-08]`. Note that **Neo4j (§5.2) turned the naming question into
an actual false-advertising holding**, so this is not purely rhetorical.

**Does OSI have authority to define "open source"?** The OSD is the de facto standard, embedded
in corporate policies, distro inclusion rules, and procurement. But OSI is a private nonprofit
with no statutory mandate, it does not hold a trademark on the generic term in most
jurisdictions, and **its own governance is currently contested** (2025 election controversy;
**January 2026 suspension of the 2026 elections**) `[V-2026-08]`. Vendors increasingly ignore
its judgment. The practical answer is that OSI approval remains the operative gate for most
tooling and policy **regardless** of the philosophical question -- which is why it matters to
engineering.

**CLA vs DCO.** CLA proponents: necessary for dual-licensing, defensive relicensing, and
provenance assurance. DCO proponents: CLAs impose contributor friction, concentrate power in a
corporate steward, and are precisely the mechanism that enabled the relicensing wave in §5.
**The disagreement is about who should hold the option to relicense** -- and §2.6 shows that
option is real and valuable.

**Is AGPL usable in a commercial product?** Some organizations use AGPL software freely,
reasoning that unmodified internal use does not trigger §13 (which the text supports `[V]`) and
that the obligation is manageable if it does. Others -- **including Google `[V-2026-08]`** --
ban it outright, reasoning that the boundary of "your version" is too uncertain to risk.
**Both are defensible risk postures, not right-or-wrong answers.** The agent should surface the
org's existing policy rather than substitute its own.

**Permissive-by-default corporate policy vs case-by-case review.** An allowlist of
MIT/BSD/Apache/ISC is cheap, fast, and rejects genuinely usable software. Case-by-case review
is accurate and expensive, and it does not scale to thousands of transitive dependencies. Most
mature orgs run a hybrid: automatic allow for a permissive set, automatic block for a banned
set, human review for the middle and for everything `unknown`.

**How much scanning is enough?** Manifest-only scanning is fast and misses vendored code,
copy-paste, and assets. Full-file-tree scanning (ScanCode, ORT) catches those and produces a
large volume of low-value findings that nobody triages. **A scanner nobody acts on is worse
than no scanner, because it manufactures the appearance of diligence** -- which is itself worth
flagging in review.

---

## 8. Anti-patterns

**Mechanism → consequence.** Each is a reviewable finding.

### Copyleft scope

1. **An AGPL-3.0 dependency in a SaaS backend, with local modifications.** §13 obligates you to
   offer Corresponding Source to *every user interacting with the service over a network* --
   including the parts you wrote that are part of "your version." → Source disclosure of
   proprietary work, or removal. **Blocker.**

2. **"We only use the AGPL service internally, so we're fine."** §13 says *all users
   interacting with it remotely through a computer network* `[V]`. Employees and other business
   units over a network are colorable users. → An obligation believed absent is live.

3. **Assuming unmodified AGPL use is a blanket safe harbor.** §13 is triggered by modification
   `[V]`, but "modification" includes patches, forks, and vendored changes -- and the scope of
   "your version" is unsettled. → A one-line patch converts a safe deployment into a disclosing
   one.

4. **A GPL-2.0-only dependency combined with Apache-2.0 code in one binary.** GPL-2.0 §7 means
   the conflicting conditions leave you unable to distribute at all `[V]`. → Not a
   "warning": the combination is undistributable.

5. **Treating a bare `GPL-2.0` identifier as equivalent to `GPL-2.0-or-later`.** `-only`
   forbids the GPL-3.0 upgrade path `[V]`. → An assumed-legal combination with GPL-3.0 code is
   actually impossible.

6. **LGPL statically linked into a signed mobile binary.** LGPL-3.0 §4(d)(1)'s shared-library
   route is unavailable, forcing §4(d)(0) -- shipping object files sufficient to relink `[V]`
   -- and §4(e) may import Installation Information the developer cannot provide. → Often
   structurally unsatisfiable. **Blocker.**

7. **Assuming "dynamic linking makes LGPL fine" without providing substitutability.** The
   obligation is that the user can *substitute a modified library* `[V]`. Dynamic linking into
   a container the user cannot alter, or a signed bundle, does not deliver that. → Obligation
   unmet despite the "right" linker flag.

8. **A copyleft header-only library treated as a mere include.** The library compiles into your
   binary; there is no linkage boundary. LGPL's ten-line thresholds `[V]` carve out only
   trivial inlines. → Full combination, full copyleft.

9. **Stripping a GPL exception during vendoring.** `GPL-2.0-only WITH Classpath-exception-2.0`
   vendored as plain GPL-2.0 code loses the very permission that made it usable `[V]`. → A
   proprietary product now contains unexception'd GPL.

10. **Recording an exception the vendored copy does not actually carry.** The inverse: metadata
    says `WITH Classpath-exception-2.0`, the copied files do not. → False clear; nobody looks
    again.

11. **Assuming a container boundary breaks copyleft.** Distributing an image is distribution,
    and a statically-linked GPL binary inside it is still a combined work. → Source-offer
    obligations you never planned for.

12. **Assuming a plugin architecture is automatically arms-length.** Whether a plugin is a
    derivative work is contested and fact-specific (§2.3) -- intimate data-structure sharing
    looks more like combination than a socket does. → A design decision made on folklore.

13. **Assuming generated code never inherits the generator's license.** It depends on whether
    the generator copies its skeleton or runtime into the output -- which is exactly why the
    **Bison exception exists** `[V]`. → Generated parsers silently carrying copyleft.

14. **Treating the syscall/`exec` boundary as universally safe.** It is well-accepted for the
    Linux kernel because the kernel explicitly says so `[V]`; that note does not generalize to
    every program you shell out to. → A boundary assumed safe by analogy.

15. **CDDL and GPL code combined** (the ZFS shape). CDDL §3.1 requires CDDL-only distribution;
    GPL requires whole-work GPL `[V]`. → Undistributable combination.

### Attribution and notice

16. **Shipping an Apache-2.0 library without propagating its NOTICE file.** §4(d) is a distinct
    obligation from the license text `[V]`. Attribution in your About box does not satisfy it
    unless the NOTICE *contents* appear. → Breach despite visibly "doing attribution."
    **Major.**

17. **Listing license *names* in a dependency table instead of shipping license *text*.** MIT
    requires the notice *"in all copies or substantial portions"* `[V]`. → Breach of the
    simplest license in the ecosystem, in almost every app that does this.

18. **Attribution present in the repo but absent from the shipped artifact.** "All copies"
    includes binaries, images, and bundles `[V]`. → The artifact users receive is
    non-compliant.

19. **Aggregating LICENSE files while skipping NOTICE files.** Most generic tooling does exactly
    this because NOTICE is not license text. → §4(d) systematically unmet across the whole
    dependency tree.

20. **Replacing upstream copyright lines with your own in an aggregated notices file.** Each
    dependency's own copyright holders must appear. → The notice file is decorative, not
    compliant.

21. **A mobile app with no attribution surface at all** -- no Settings acknowledgements, no
    About box, no `licenses.txt`. → Every permissive dependency's notice obligation is unmet
    simultaneously.

22. **Building a new pipeline on CocoaPods' acknowledgements plist.** CocoaPods is in
    maintenance mode as of 2024-08-13 `[V-2026-08]`. → A compliance surface with a scheduled
    end-of-life.

### Source-available and policy

23. **Upgrading a BUSL/SSPL/ELv2 dependency across the relicense boundary in a routine bump.**
    Redis 7.2→7.4 and Elasticsearch 7.10→7.11 changed license, not just version `[V-2026-08]`.
    → A dependabot PR converts a permissive dependency into a restricted one, invisibly.

24. **Assuming a relicense is retroactive.** It is not: Redis ≤7.2 remains BSD-3, ES ≤7.10.2
    remains Apache-2.0 `[V-2026-08]`. → Panic-removing a dependency that was never affected.

25. **Assuming a BUSL Change Date applies repo-wide.** The clock is **per-version** `[V]`. → A
    repo simultaneously contains restricted and already-converted releases; treating it as one
    state is wrong in both directions.

26. **Stripping a Commons Clause and calling the result open source.** *Neo4j* held this was
    impermissible and constituted false advertising (§5.2). → Contract and false-advertising
    exposure, not just a licensing dispute.

27. **Shipping SSPL code in a hosted service.** §13's "Service Source Code" reaches management,
    monitoring, backup, and hosting software `[V]`. → Effectively your entire operational
    stack. **Blocker.**

28. **Treating "source available" as a synonym for "open source" in policy or marketing.** The
    vendors themselves concede otherwise (MongoDB, Redis) `[V-2026-08]`. → Policy gates keyed on
    OSI approval silently mis-classify.

29. **Adopting an FSL-licensed dependency recorded as `FSL-1.1-Apache-2.0`.** That identifier is
    not in SPDX; the real ones are `FSL-1.1-ALv2` / `FSL-1.1-MIT` `[V-2026-08]`. → Tooling
    treats it as unknown or fails to match a policy rule.

### Tooling and process

30. **`cargo-deny` with an allowlist but a lowered `confidence-threshold`, or `clarify` entries
    nobody reviewed.** v2 is deny-by-default `[V-2026-08]`, so the modern failure is not
    "unknown wasn't denied" -- it is unknowns being *waved through* by configuration. → Crates
    pass on an assertion nobody verified.

31. **Copying `deny.toml` config from a pre-2024 blog post.** `copyleft`, `unlicensed`,
    `default`, `allow-osi-fsf-free`, and `deny` were **removed in 0.16.0 and now hard-error**
    `[V-2026-08]`. → CI fails confusingly, or someone "fixes" it by deleting the checks.

32. **Relying on `dependency-review-action` to catch unlicensed dependencies.** Unlicensed
    dependencies produce **only an informational notice; they do not fail the action**
    `[V-2026-08]`. → A pipeline that "checks licenses" passes unlicensed code silently.

33. **Using `deny-licenses` in `dependency-review-action`.** Deprecated in favor of
    `allow-licenses` `[V-2026-08]` -- and a denylist fails open anyway. → Unknown licenses pass.

34. **Scanning only direct dependencies.** You ship the transitive closure. → A permissive
    direct dependency hides a copyleft transitive one.

35. **Scanning only the manifest, never the file tree.** Vendored directories, copy-pasted
    files, and bundled assets are invisible to manifest-based tools. → The riskiest code is the
    least visible.

36. **Gating CI on version changes but not license changes.** See #23. → The one thing you most
    need to catch is the one thing the gate does not look at.

37. **A `.ttf` or `.otf` in the repo with no license file.** Font EULAs carry real per-app and
    per-pageview fees. → Unknown, potentially expensive, obligation shipped in the binary.

38. **Subsetting an OFL font and keeping its Reserved Font Name.** The OFL FAQ states subsetting
    *is* modification `[V-2026-08]`, and clause 3 forbids the RFN on modified versions `[V]`. →
    A live, widespread violation in ordinary web build pipelines.

39. **Relicensing an OFL font under the app's blanket proprietary terms.** Clause 5 requires it
    stay *"entirely under this license"* `[V]`. → Breach of the font license regardless of the
    app's terms.

40. **Treating a multi-license package as single-licensed.** Font Awesome Free is CC-BY-4.0 +
    OFL-1.1 + MIT and reports as `NOASSERTION` `[V-2026-08]`; Lucide is ISC + MIT `[V-2026-08]`.
    → Only one of several obligations gets satisfied.

41. **Using a CC-BY-SA asset in a product UI.** §3(b) requires adaptations be ShareAlike-licensed
    `[V]`. → A copyleft obligation on design work, from an image.

42. **Any CC-BY-NC asset in a commercial product.** NonCommercial is categorically incompatible
    with a commercial product. → Immediate breach. **Blocker.**

43. **Pasting a substantial Stack Overflow snippet without attribution.** Contributions are
    CC-BY-SA `[U on version dates]` -- attribution *and* ShareAlike. → Unattributed copyleft in
    the codebase.

44. **Assuming a CC0 or MIT icon set licenses the trademarks it depicts.** Simple Icons says so
    explicitly `[V-2026-08]`; Apache-2.0 §6 excludes trademarks `[V]`. → Trademark exposure from
    a "permissively licensed" logo.

45. **Forking without renaming.** Trademark is not granted by any of these licenses `[V]`. →
    The trademark claim that actually forced every major fork to rename (§3.7).

46. **Relying on Copilot's public-code filter as an indemnity precondition.** True until
    **2026-04-03**, when Microsoft removed the requirement `[V-2026-08]`. → Policy built on a
    stale fact; conversely, **AWS still does** condition its indemnity on filters, so
    generalizing across vendors is also wrong.

47. **Relicensing a community project without a CLA.** Requires **every** contributor's consent
    `[V]`. → Either an impossible consent-gathering exercise, rewriting holdouts' code (as VLC
    did `[V-2026-08]`), or an invalid relicense.

48. **Adding a CLA late to an established project.** Existing contributions remain under the
    original inbound terms. → The CLA does not retroactively enable the relicense it was added
    for.

49. **Running a scanner nobody triages.** Unreviewed findings accumulate into noise. → Worse
    than no scanner: it manufactures the appearance of diligence while the real obligations go
    unmet. Flag this as a process finding.

50. **Treating "no findings" as legal clearance.** Scanners miss vendored code, assets,
    snippets, and anything they cannot identify. → False confidence at exactly the point where
    the residual risk concentrates.

---

## 9. Review checklist

**Dependency changes**
- New dependency: license identified? On the allowlist? Transitive closure checked?
- Version bump: **did the license expression change?**
- Any `unknown`/`NOASSERTION`/`LicenseRef-*` in the tree → treat as blocking until resolved
- Bare `GPL-2.0`-style identifiers → resolve `-only` vs `-or-later`
- `WITH <exception>` present in metadata *and* in the actual vendored files?

**Shipping obligations**
- Does the build produce an aggregated notices artifact, and does it include **NOTICE** contents
  (not just LICENSE text)?
- Is that artifact reachable by a user of the **shipped** binary?
- If GPL is distributed: who holds the corresponding source, including build scripts, for three
  years?
- If AGPL is served: is there a prominent source offer, and has anyone confirmed whether the
  code is modified?

**Platform**
- Copyleft in an app-store binary → §3.5; iOS is a blocker pending counsel, Play is a major
  finding
- LGPL + static linking → §3.4
- Fonts and icon sets → separate license per asset class; check for a declared Reserved Font
  Name and for subsetting

**Policy**
- Is the gate an **allowlist** (correct) or a denylist (fails open)?
- Does CI **fail** on unknown licenses, or merely warn?
- Is `confidence-threshold` at its default, and are `clarify` entries reviewed?

**Escalate to counsel** (do not resolve in review): derivative-work questions, any
source-available license, any copyleft in a distributed proprietary product, license changes,
patent or trademark questions, CLA/assignment questions, and anything where the answer depends
on jurisdiction.

---

## 10. Known gaps in this file

Stated plainly so nobody mistakes silence for verification:

- **The FSF GPL FAQ and license-list were retrieved via web.archive.org mirrors**, not the
  gnu.org origin (which was intermittently unreachable). Content matched expected canonical
  text, but if you need origin-verified citations, re-fetch from gnu.org.
- **opensource.org's site pages were unreachable** (HTTP 403). OSD prose and OSI commentary are
  `[U]`. **OSI approval *statuses* are verified** -- taken from SPDX `licenses.json`'s
  `isOsiApproved` field, which is authoritative and machine-readable (§1.1).
- **No FSF license-list entry for SSPL was found.** OSI non-approval is verified via SPDX; **do
  not assert an FSF position on SSPL.**
- **WebFetch paraphrases license text.** Every verbatim quote in this file came from raw text
  (`curl` or `raw.githubusercontent.com/spdx/license-list-data`). If you extend this file, do
  not quote a license from a summarizing fetch -- it will silently reword the clause.
- **Case law is now the best-sourced section**, verified from primary court documents (orders,
  judgments, dockets) rather than secondary reporting. Method note for future extension:
  CourtListener's `/docket/` HTML returns 403, but its **v4 search API works unauthenticated**
  and `storage.courtlistener.com` PDFs download fine. State cases (Vizio) appear on neither.
  Still `[U]`: Neo4j's second appeal (9th Cir. 24-5538) and certiorari; settlement terms in
  Artifex, XimpleWare, and the BusyBox suits; the Travis County outcome in *Versata*; the
  Entr'ouvert *pourvoi* number; the exact ESXi version that dropped vmklinux; §1202 postures in
  *Intercept v. OpenAI* and *Advance Local Media v. Cohere*.
- **Two cases are live and will move.** *Doe v. GitHub* was argued 2026-02-11 and is undecided;
  *SFC v. Vizio* has no final judgment. **Re-check both before citing.**
- **Stack Overflow's CC-BY-SA version dates** and its AI-licensing deals are `[U]`
  (stackoverflow.com blocked).
- **Foundry font EULA specifics** (MyFonts pageview tiers, Adobe app-embedding) are `[U]`.
  **Do not guess these; read the actual EULA.**
- **Fedora's CC0-for-code policy** is `[U]` (site blocked bot access).
- **LGPL-on-iOS practitioner guidance** is thin -- the mechanism is verified, the "what teams
  actually do" is not.
- **CISA's 2025 SBOM minimum elements** appear to still be draft `[U]` (cisa.gov blocks fetch).
- Commercial scanner capabilities (FOSSA, Snyk, Black Duck format support) are `[U]`.

**Re-verification cadence:** license-list versions and tool status drift within months; vendor
licenses and litigation status drift within weeks. Treat anything marked `[V-2026-08]` as
needing a recheck if the current date is more than ~6 months later.
