---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Licensing: Cases, Enforcement, and Sources

Companion to `~/.claude/rules/licensing-and-oss.md`. Pull this in when a finding turns on enforceability, remedies, or jurisdiction. **Same standing caveat: engineering guidance, not legal advice**, and the verification markers carry the same meaning.

---

## 5. Canonical incidents and cases

**All case outcomes below are engineering-relevant summaries, not legal analysis. Statuses
change. Verify before relying.**

### 5.1 Enforcement litigation

All case data below was verified from **primary court documents** (orders, judgments, dockets)
`[V-2026-08]` unless marked otherwise. Docket numbers are included so counsel can pull the
record directly.

**The single most important doctrinal fact in this section:** US courts consistently hold that
open-source license claims survive copyright preemption **as contract claims**, because there is
an "extra element" -- the right to receive source code -- that copyright does not provide. That
holding runs *Versata* → *Artifex* → *Vizio*, and it is why "the GPL is just a license, not a
contract" is wrong in US practice.

**BusyBox / SFLC (2007-2010)** `[V-2026-08]`
First US GPL suits. *Andersen v. Monsoon Multimedia*, S.D.N.Y. **07-CV-8205**, filed
**2007-09-20**; settled with source release plus an undisclosed payment. Then Xterasys and
High-Gain Antennas (Nov 2007), Verizon/Actiontec (Dec 2007), Bell Microproducts and SuperMicro
(2008). **2009-12-14**: fourteen defendants including Best Buy, Samsung, JVC, Western Digital,
and Bosch. **2010-08-03**: default judgment against Westinghouse, **$90,000 trebled plus
$47,865 in fees**.

**Rob Landley's critique** (*"The Rise and Fall of Copyleft,"* Ohio LinuxFest 2013) `[V-2026-08]`,
verbatim: *"Something like a dozen lawsuits, not one line of code added to busybox
repository."* He conceded that good enforcement exists (Linksys) but argued the suits pushed
companies out of GPL projects entirely -- an influence on Android's no-GPL-in-userspace posture.

**Why this matters normatively:** that critique is the direct ancestor of the modern norm.
SFC and FSF's **Principles of Community-Oriented GPL Enforcement** now put compliance over
damages, treat litigation as a last resort, and refuse payment in exchange for overlooking
violations. **Most GPL enforcement still resolves privately, which is why so little case law
exists.**

**Artifex Software v. Hancom** `[V-2026-08]` -- GPL as an enforceable contract
N.D. Cal. **3:16-cv-06982-JSC**, Magistrate Judge Jacqueline Scott Corley.

**Correction to the common telling:** the famous **2017-04-25** order (Dkt. 32) was a
**motion-to-dismiss denial**, not summary judgment. On mutual assent, verbatim:

> Defendant contends that Plaintiff's reliance on the unsigned GNU GPL fails to plausibly
> demonstrate mutual assent... **Not so.** The GNU GPL... provides that the Ghostscript user
> agrees to its terms if the user does not obtain a commercial license... These allegations
> sufficiently plead the existence of a contract.

Quoting *Jacobsen v. Katzer*, 535 F.3d 1373, 1379 (Fed. Cir. 2008): *"[t]he lack of money
changing hands in open source licensing should not be presumed to mean that there is no
economic consideration."* A second order (Dkt. 54, **2017-09-12**) denied partial summary
judgment, rejecting the "free license means zero damages" argument under Cal. Civ. Code § 3358.

**Settled ~Dec 2017; dismissed with prejudice 2018-01-17. Terms never disclosed -- do not
assert a settlement figure.**

**SFC v. Vizio** `[V-2026-08]` -- the third-party beneficiary theory, and a major 2025 setback
Cal. Super. Ct., Orange County, No. **30-2021-01226723-CU-BC-CJC**, Dept. C33, Judge Sandy N.
Leal. Filed **2021-10-19**.

**The theory:** SFC sues **as a purchaser and third-party beneficiary** of GPLv2/LGPLv2.1 as a
**contract**, seeking **specific performance** (the source code) -- *not* as a copyright holder,
and *not* for damages. If it succeeds, **downstream recipients, not just copyright holders,
can demand source.** That would substantially widen who can enforce copyleft.

**Remand** (C.D. Cal. No. 8:21-cv-01943-JLS-KES, Judge Staton, **2022-05-13**), verbatim:

> There is an extra element to SFC's claims because SFC is asserting, as a third-party
> beneficiary of the GPL Agreements, that it is entitled to receive source code... There is no
> right to receive certain works -- or source code in particular -- under the Copyright Act...
> the right to receive the source code would appear to be "the very opposite" of those
> exclusive rights.

**Procedural chain:**
- **2023-12-29** -- Vizio's first summary-judgment motion **denied**
- **2024-03-26** -- SFC's first motion **granted in part**. It won only on Issue 2 (killing
  Vizio's preemption defense) and **lost Issue 1**: Vizio produced evidence that the **FSF "did
  not intend for third parties to enforce the rights under the license agreement,"** creating a
  **triable issue on third-party beneficiary status that has never been resolved**
- **2025-12-23** -- **Vizio's second motion for summary adjudication GRANTED**, verbatim:

> The language of the Agreements is unambiguous. It does not impose the duty which is the
> subject of this motion... nothing in the language of the Agreements requires Vizio to allow
> modified source code to be reinstalled on its devices while ensuring the devices remain
> operable after the source code is modified... The absence of such language is dispositive.

**Read that holding narrowly and precisely.** It says **GPLv2 contains no anti-tivoization
requirement** -- which is correct and unsurprising, since anti-tivoization is a **GPLv3**
feature (§1.3). It did **not** reject the third-party beneficiary theory. `[DISPUTED]` SFC
argues publicly that the ruling addressed *"a position that SFC has not actually taken"* and
predicts others will *"claim this ruling reaches further than it says"*; Judge Leal disagreed,
finding SFC's interrogatory response *"can be reasonably construed to include the issue."* Both
readings are in the same public PDF.

**Status: no trial, no final judgment, no settlement as of 2026-08-26.** Trial was continued
repeatedly and converted from jury to **bench trial**. SFC's case page (last updated
2026-02-27) still lists trial as "late 2026." This is a state case, so it does **not** appear on
CourtListener. **Do not describe this as a win for either side.**

**Versata v. Ameriprise** `[V-2026-08]` -- the origin of the "extra element" holding
**Correction: this was federal, not state.** W.D. Tex. **1:14-cv-00012-SS**, Judge Sam Sparks.
The **2014-03-11** order (2014 WL 950065) did three things at once: granted Ameriprise summary
judgment (Versata's own claim preempted), **denied** Versata summary judgment (**the GPL
counterclaim is NOT preempted** -- the "extra element" holding later quoted by *both* Artifex
and the Vizio remand), and remanded to Travis County. The court **expressly declined** to decide
GPL third-party-beneficiary standing. Travis County outcome `[U]`.

Ameriprise conceded it could not sue for infringement *"because copyrights must be enforced by
the copyright holder, not an interested third party"* -- **precisely the gap SFC's Vizio theory
tries to fill.**

**XimpleWare** `[V-2026-08]` -- GPL as a copyright *sword*
Two N.D. Cal. cases, both filed 2013-11-05. In No. 3:13-cv-05160-SI (Judge Illston), the
**2014-02-04** order (Dkt. 61) denied dismissal: distribution to non-employee contractors was
*"outside the scope of the GPL."* Applying *Jacobsen*, a nonexclusive licensee is normally
suable only in contract, **but where the license is limited in scope and the licensee exceeds
it, the licensor can sue for infringement.** Both settled Feb 2015; terms undisclosed.

**Hellwig v. VMware** `[V-2026-08]` -- why enforcement is hard
Filed **March 2015**, Landgericht Hamburg. **Dismissed 2016-07-08 on German evidentiary rules**
about documenting Hellwig's specific contributions -- **not on the merits.** Hamburg Higher
Regional Court **affirmed 2019-02-28**, again procedurally. Hellwig declined further appeal
**2019-04-02**. VMware separately committed to **removing vmklinux from vSphere** -- compliance
by deletion, not by releasing source.

**Lesson: proving which contributor owns which lines of a large codebase is a genuine practical
barrier to enforcement. Net result: no substantive precedent on derivative works under GPL.**

**Entr'ouvert v. Orange** `[V-2026-08]` -- the European counterpoint
**Correction: the Cour de cassation ruling is 2022-10-05, not 2021.** Chain: TGI Paris
**2019-06-21** (dismissed; contract-only) → Cour d'appel **March 2021** (confirmed, on
*non-cumul des responsabilités*) → **Cour de cassation 2022-10-05** (*casse et annule*,
rejecting non-cumul) → **Cour d'appel de Paris 2024-02-14, No. 22/18071**, condemning Orange
for **contrefaçon** (infringement).

**Damages: €860,000 minimum** -- the widely-cited ~€500,000 figure is only the first line item
(€500k economic + €150k moral + €150k Orange's profits + €60k art. 700 + costs). No *pourvoi*
filed; closed.

**This is the jurisdictional split in one case:** the CJEU's *IT Development v. Free Mobile*
(Dec 2019) held license violation **can be infringement**, where US courts route the same facts
through **contract**. Remedies and damages differ enormously. **This is why "which jurisdiction"
is a real question, not a formality.**

**Kernel Enforcement Statement and GPL Cooperation Commitment** `[V-2026-08]`
Kernel commit **9ed95129f**, authored **2017-10-04**. **95 signatories initially; 109 today**
(including Torvalds, Kroah-Hartman, Molnar, Viro, Miller). It adopts GPLv3's cure provisions as
**additional permissions** on GPLv2:

> Notwithstanding the termination provisions of the GPL-2.0, we agree... to adopt the following
> provisions of GPL-3.0 as additional permissions under our license with respect to any
> non-defensive assertion of rights under the license.

Provisional reinstatement on cessation; permanent if not notified within **60 days**, or if
cured within **30 days** of first notice.

**GPL Cooperation Commitment**: launched **Nov 2017** (Red Hat, IBM, Google, Facebook), expanded
March 2018. Now **63 companies and 475 individuals**. Legally a **binding additional
permission**, not a statement of intent: *"irrevocable, and binding and enforceable against me
and assignees of or successors to my copyrights."*

**Practical consequence: an inadvertent violation, promptly cured, is much less likely to become
existential -- but only with parties who adopted the commitment.** Check whether the copyright
holder is a signatory.

**SFC v. Bambu Lab (2026-05-18)** `[V-2026-08]` -- current, and pre-litigation
Not a lawsuit. Two alleged AGPLv3 violations: Bambu Studio combining AGPL code with proprietary
networking libraries without corresponding source; and Bambu demanding takedown of a modified
OrcaSlicer fork, which SFC characterizes as imposing **"further restrictions" barred by AGPLv3
§10** -- the same doctrinal hook as *Neo4j*, from the opposite direction. SFC responded with
engineering (a reverse-engineering project) rather than litigation, consistent with its stated
principles.

### 5.2 Neo4j and the "open source" naming question

**Neo4j v. PureThink / Suhy** `[V-2026-08]`, verified against the opinion and judgment.

Ninth Circuit **No. 21-16029**, memorandum disposition **amended 2022-03-14** (original
2022-02-18). **NOT FOR PUBLICATION -- it is non-precedential.** Say "persuasive," not
"binding."

**Holding -- removing the Commons Clause was NOT permitted**, verbatim:

> The representation that ONgDB is a "free and open-source" version of Neo4j® EE was
> **literally false**, because Section 7 of the Sweden Software License **only permits a
> downstream licensee to remove "further restrictions" added by an upstream licensee** to the
> original work.

**This is the operative engineering rule, and it is narrow but sharp:** AGPL §7 lets you strip
restrictions that *others* added downstream. It does **not** let you strip restrictions the
**original licensor** attached. Do not assume a Commons Clause is severable.

**2024-07-22 final judgment:** **$597,000** actual damages plus **$57,288** prejudgment
interest, less a stipulated $26,000, joint and several, plus a **permanent injunction** barring
Suhy from stating that removal of the Commons Clause is lawful, or that the FSF or any agency
confirmed otherwise.

A second appeal exists (9th Cir. **24-5538**, filed 2024-09-11); outcome and certiorari `[U]`.

**Two takeaways:** a copyleft license plus a proprietary rider is **not** freely severable; and
**calling something "open source" when it is not carries false-advertising liability** -- the
sharpest practical consequence of the OSI-approval question in §6.

### 5.3 Google v. Oracle -- and what it did NOT settle

**Google LLC v. Oracle America, Inc.** `[V-2026-08]`, verified from the slip opinion. No.
**18-956**, argued 2020-10-07, **decided 2021-04-05**, **6-2**, Breyer, J. (Barrett took no
part); Thomas dissenting, joined by Alito.

**Held:** copying **~11,500 lines of declaring code** (0.4% of 2.86M lines) was **fair use**.

**What it did NOT settle**, verbatim:

> We shall assume, **but purely for argument's sake**, that the entire Sun Java API falls
> within the definition of that which can be copyrighted.

**API copyrightability remains formally open in the US.** The Federal Circuit's contrary holding
was left standing but unreviewed. **"The Supreme Court said APIs aren't copyrightable" is
wrong**, and a reimplementation strategy built on that belief is built on sand. What you
actually have is a strong fair-use precedent for reimplementing an API for interoperability,
reasoned heavily on the facts.

### 5.4 Doe v. GitHub (Copilot) -- argued, undecided

`[V-2026-08]` verified from the appellants' brief and live dockets. **Doe v. GitHub, Inc.**,
N.D. Cal. **4:22-cv-06823**, Judge Jon S. Tigar. Filed 2022-11-03.

- **2024-06-24** -- DMCA §1202(b)(1)/(b)(3) claims **dismissed**; the SAC allegations *"failed
  to meet the DMCA's identicality requirement."* Even "nearly verbatim copying" was held
  insufficient.
- **2024-09-27** -- certified for interlocutory appeal under **28 U.S.C. §1292(b)**; trial
  proceedings **stayed**.
- **9th Cir. No. 24-7700.**
- **Breach of contract / open-source license claims were NEVER dismissed** -- they are stayed
  pending appeal.

**Status: ARGUED AND SUBMITTED 2026-02-11** before Thomas, Miller, and Blumenfeld (D.J.).
**NO DECISION as of 2026-08-26** -- verified across all 125 docket entries and both CA9 opinion
and memoranda searches. Note `githubcopilotlitigation.com` is **stale since 2025-04-09** and is
not a reliable status source.

**Two findings that change the story:**
1. **Both defendant groups abandoned strict identicality on appeal.** OpenAI: the statute *"does
   not require identicality."* GitHub/Microsoft: no *"rigid identicality requirement."* They
   recast it as a "weighty factor"; plaintiffs argue waiver. **The panel is unlikely to bless
   strict identicality when nobody defends it.**
2. **The EFF filed for the *defendants*** (with Public Knowledge), supporting affirmance and
   *keeping* identicality as an anti-trolling protection. Also for affirmance: Chamber of
   Progress + CCIA, Authors Alliance, and 16 IP professors (Lemley, Samuelson, Tushnet, Sag,
   Jaszi). For appellants: Authors Guild, AAP, News/Media Alliance, ACT. **This is not a
   clean "open source vs. AI" alignment.**

**The §1202(b) identicality split is real and no circuit has resolved it** `[V-2026-08]`.
*Requiring* identicality: *Kirk Kara* (C.D. Cal. 2020); *Crowley v. Jones* (S.D.N.Y. 2022);
**NYT v. Microsoft**, 2025 WL 1009179 (S.D.N.Y. 2025-04-04); **Kadrey v. Meta**, 2025 WL 1786418
(N.D. Cal. 2025-06-27). *Rejecting* it: **Oracle Int'l v. Rimini St.**, 2023 WL 4706127 (D. Nev.
2023-07-24) -- the closest analogue, endorsing §1202(b) liability for *"substantially similar"*
**source code**; *GC2 v. IGT* (N.D. Ill. 2019); *Doe v. GitHub* is the first circuit-level test.

**Note on scope:** *Raw Story v. OpenAI* (2d Cir. 25-1756, argued 2026-03-18) turns on **Article
III standing, not identicality**, so it will not resolve the split.

**The durable engineering takeaway:** the plaintiffs' strongest surviving theory is **breach of
the open-source licenses as contracts**, not copyright infringement. That is the same
contract-flavored framing as *Artifex*, *Versata*, and *Vizio* -- **four separate lines of
litigation converging on treating open-source licenses as enforceable contracts.**

### 5.5 The relicensing waves

`[V-2026-08]` unless noted. **Several vendors reverted; several did not. Check current state
before acting on any of these.**

| Project | Change | Date | Reverted? |
|---|---|---|---|
| **MongoDB** | AGPL-3.0 → **SSPL-1.0** | 2018-10-16 | **No.** Still SSPL. MongoDB's own FAQ concedes *"not considered open source by the OSI"* |
| **Elastic** | Apache-2.0 → SSPL + ELv2 (in 7.11) | 2021-01-14 | **Partially.** **AGPL-3.0 added 2024-08-29.** Now a genuine **tri-license**: AGPL-3.0-only OR SSPL-1.0 OR Elastic-2.0 (`x-pack/` stays ELv2-only) |
| **Grafana** | Apache-2.0 → **AGPL-3.0** | 2021-04 | **No** -- and note this is a *copyleft tightening*, not an exit from open source. AGPL is OSI-approved |
| **Akka** | Apache-2.0 → BSL | 2022-09-07 | **No.** Still BUSL-1.1 (Change Date 2029-08-12 → Apache-2.0) |
| **HashiCorp** | MPL-2.0 → **BUSL-1.1** (8 products incl. Vagrant) | 2023-08-10 | **No.** 4-year term → MPL-2.0. **IBM acquisition closed 2025-02-27**; the LICENSE file now names IBM as Licensor. Libraries/SDKs stayed MPL-2.0 |
| **Sentry** | BSL (2019-11-06) → **FSL** | 2023-11-17 | n/a -- FSL converts to Apache-2.0 after 2 years |
| **Redis** | BSD-3 → RSALv2 + SSPL (in 7.4) | 2024-03-20 | **Partially. AGPLv3 added 2025-05-01** in Redis 8. Now tri-licensed RSALv2 / SSPLv1 / AGPLv3. **Redis ≤7.2 remains BSD-3** |
| **CockroachDB** | BUSL → **proprietary CockroachDB Software License** | announced 2024-08-15, landed 2024-10-01 (v24.3.0) | **No -- moved further from open source.** Free tier under $10M revenue with mandatory telemetry |
| **Sourcegraph** | Apache-2.0 → closed | 2024-09-30 | Repo renamed `sourcegraph-public-snapshot` and archived |
| **Puppet / Perforce** | closed *development* (license unchanged) | Nov 2024 → early 2025 | Fork: **OpenVox** (first release 2025-01-21) |
| **MinIO** | AGPL-3.0 retained; **pre-compiled community binaries withdrawn** | repo archived 2026-04-25 | Same license, withdrawn convenience |
| **NATS / Synadia** | proposed BSL fork | 2025-04-25 | **YES -- fully reverted 2025-05-13** after CNCF pressure; stays Apache-2.0, **trademarks transferred to CNCF** |

**The forks, all at the Linux Foundation** `[V-2026-08]`:
- **OpenTofu** (from Terraform): manifesto 2023-08-10, public 2023-08-25, renamed and joined LF
  2023-09-20. **MPL-2.0**, v1.12.6 (2026-08-19), actively developed.
- **Valkey** (from Redis): announced 2024-03-28. **BSD-3-Clause**, v9.1.1. Founders included
  Madelyn Olson (AWS, ex-Redis core). Backed by AWS, Google Cloud, Oracle, Ericsson, Snap.
- **OpenSearch** (from Elasticsearch 7.10.2): 2021; moved to the **OpenSearch Software
  Foundation** (LF) September 2024. **1 billion+ downloads, +78% YoY**; IBM joined as a Premier
  member 2025-11-11.

**HashiCorp v. OpenTofu was a cease-and-desist, never a lawsuit** `[V-2026-08]`. HashiCorp's
counsel alleged (2024-04-03) that OpenTofu had re-labeled BSL code as MPL; OpenTofu responded
publicly (2024-04-11) tracing the disputed code to **older MPL-2.0 code both projects derived
from**. No suit was filed. The matter went quiet.

**Three patterns worth internalizing:**
1. **Partial retreat under fork pressure** (Elastic, Redis) -- both re-added an OSI-approved
   option *after* the fork had already taken the hyperscalers. Neither killed the fork.
2. **No retreat at all** (MongoDB, HashiCorp/IBM, Akka, Confluent) -- and CockroachDB moved
   further away.
3. **Genuine reversal is rare and needs outside leverage** -- NATS is the clean case, and it
   took a CNCF confrontation plus a trademark transfer.

**The frontier has moved past relicensing.** Puppet closed *development* while leaving
Apache-2.0 in place; MinIO kept AGPL but withdrew *binaries*; Sourcegraph simply stopped
publishing. **Watching SPDX identifiers alone would have missed all three.** Availability and
maintenance are now as much a dependency risk as license text.

### 5.6 2025-2026 developments

`[V-2026-08]` -- items with live deadlines:

- **EU CRA Article 14 reporting obligations apply from 2026-09-11.** Main obligations
  2027-12-11. See §4.1 for what the SBOM requirement actually is (and is not).
- **SCOTUS denied cert in *Thaler* 2026-03-02** -- human authorship settled at the appellate
  level.
- **Copyright Office Part 3 (Generative AI Training) is still pre-publication**, 15+ months on.
- **Microsoft dropped the duplicate-detection requirement for Copilot indemnity, effective
  2026-04-03** (§4.8) -- the single most-stale fact in circulation.
- **GitHub Copilot terms replaced 2026-03-05** by the GitHub Generative AI Services Terms.
- **Ecma TC54 standardization of the BOM stack** (ECMA-424/427/428), Dec 2025.
- **SPDX License List 3.28.0**, 2026-02-20.

**OSI's Open Source AI Definition (OSAID)** `[V-2026-08]` -- relevant when a dependency is a
*model*, not code.
- **Version 1.0, released 2024-10-28. No 1.1 or later exists.**
- Grants four freedoms (Use, Study, Modify, Share) and requires three components: **Data
  Information**, **Code**, **Parameters**.
- **It does not require the training data itself.** "Data Information" is *"sufficiently detailed
  information about the data used to train the system so that a skilled person can build a
  substantially equivalent system."* Four data classes are permitted, including **unshareable
  non-public** data. This compromise is the whole controversy.
- **[DISPUTED]** The **FSF** announced (2024-10-22, six days before OSAID 1.0) competing criteria
  that *"will require the software, as well as the raw training data and associated scripts"* to
  grant the four freedoms. Whether FSF ever published final criteria is `[U]`.
- **Debian has no adopted position.** A General Resolution proposing that models without
  training data are not DFSG-compliant was **withdrawn** `[V-2026-08]`. Do not cite "Debian's
  position" as settled.
- Counter-definitions: **Open Weight Definition** (still **v0.3**, 2025-01-21 -- pre-1.0);
  **Model Openness Framework** (LF AI & Data, a graded class-1/2/3 score rather than pass/fail).
- OSI's own validation: **passed** -- Pythia, OLMo, Amber/CrystalCoder, T5. **Failed** --
  Llama 2, Grok, Phi-2, Mixtral.

**Model licenses in practice** `[V-2026-08]` -- the review-relevant part:
- **Llama** fails the OSD on at least four independent grounds: the **700M MAU** clause (OSD #5),
  the incorporated Acceptable Use Policy's field-of-endeavor bans (OSD #6), the "Built with
  Llama" naming mandate, and an **EU-domicile carve-out** for multimodal rights (OSD #5).
- **OpenRAIL / RAIL** licenses carry behavioral-use restrictions that violate OSD #6, and the
  restrictions are **viral** -- derivatives must carry them forward.
- **"DeepSeek is MIT" depends on which model.** DeepSeek-V3 shipped **dual**: MIT for code,
  plus a separate RAIL-style **DeepSeek License Agreement** for the *weights* (PRC law, use
  restrictions). **R1 and V3.1 are MIT for the weights.** R1 *distills* inherit their bases --
  Qwen-derived are Apache-2.0, **Llama-derived carry Llama licenses.**
- **"Mistral is Apache-2.0" is half true.** Small models are Apache-2.0; **Mistral Large is
  the non-commercial Mistral Research License.**
- **Gemma** is a custom gated license -- gating alone conflicts with OSD #7.
- Genuinely Apache-2.0: Qwen3, OLMo-2, Pythia, gpt-oss.
- **OSI governance turmoil**: 2025 board elections drew criticism over excluded candidates; in
  **January 2026 the board voted to redesign board selection and suspend the 2026 elections.**
  Relevant because OSI's authority to define "open source" (§6) rests partly on its legitimacy.
- **MinIO archived its community repo 2026-04-25.**

---

---

## 7. Thought leaders and canonical sources

**Verify roles before citing -- several commonly-repeated attributions are wrong.** `[V-2026-08]`
unless noted.

**Organizations**
- **Free Software Foundation** -- authors the GPL family. Its **GPL FAQ**
  (https://www.gnu.org/licenses/gpl-faq.html) is the single most-cited practical resource in the
  field. gnu.org was **unreachable from this research environment**; read it directly.
  **Ian Kelling** became FSF board president in 2025.
- **Software Freedom Conservancy (SFC)** -- fiscal host and the most active current GPL
  enforcer (the Vizio case, §5.1). **Karen Sandler**, Executive Director (attorney; "cyborg
  lawyer" for medical-device software; co-organizes Outreachy; formerly GNOME Foundation ED and
  SFLC General Counsel). **Bradley Kuhn (Kühn)**, Policy Fellow & Hacker-in-Residence -- FSF
  Executive Director 2001-2005, **invented the AGPL network-services clause**, co-editor of
  copyleft-next. **Denver Gingerich**, Director for Digital Autonomy, runs compliance and
  right-to-repair enforcement. **Rick Sanders**, General Counsel.
- **Software Freedom Law Center (SFLC)** -- **Eben Moglen**, President and Executive Director;
  Professor of Law at Columbia; **with Stallman conceived and drafted GPLv3** and its public
  comment process. Co-directors include **Diane M. Peters** (GC of Creative Commons; formerly
  GC of OSDL) and **Daniel Weitzner** (MIT).
- **Open Source Initiative (OSI)** -- maintains the **Open Source Definition** and the license
  approval process. **Corrections:** **Stefano Maffulli stepped down as ED in September
  2025**; **Deborah Bryant** is Interim ED. **Pamela Chestek is NOT the board chair** --
  **Tracy Hinds** is chair; **McCoy Smith** chairs the License Committee.
- **SPDX** -- Linux Foundation. ISO/IEC 5962:2021 covers SPDX 2.2.1. Now spans profiles
  including **AI and Dataset**.
- **Linux Foundation OpenChain** -- **ISO/IEC 5230** (license compliance, Edition 1, published
  2020-12-15, re-confirmed current 2026-07-30) and **ISO/IEC 18974** (security assurance,
  **:2023**). Both free from ISO. The certifiable process standards for a compliance program.
- **FSFE** -- **REUSE** specification and tool; the **FLA-2.0**.
- **Creative Commons** -- the CC license suite; explicitly recommends against CC for software.
- **Blue Oak Council** -- the permissive-license **rating list** (Gold/Silver/Bronze/Lead) and
  the Blue Oak Model License.

**Individuals**
- **Heather Meeker** -- the standard practitioner author on open-source licensing (*Open
  (Source) for Business*; *Technology Licensing*). **Verified drafter of the Commons Clause**
  `[V]`. Widely credited with BUSL and FSL drafting `[U -- not confirmed]`. Blog "Copyleft
  Currents," active through 2026; new book *From Project to Profit*; launched **Cossmology**, a
  directory of commercial-open-source companies.
- **Van Lindberg** -- *Intellectual Property and Open Source* (O'Reilly, 2008), the standard
  engineer-facing IP text. Current role `[U]`.
- **Kyle Mitchell** -- license drafting; associated with the **Blue Oak Model License** and
  **PolyForm**. Note both projects credit their councils rather than him individually
  `[V-2026-08]`; "associated with" is the safe phrasing.
- **Luis Villa** -- **now VP Legal for Product and Policy at Sonar** (Tidelift was acquired
  by Sonar). Tidelift co-founder and GC; **led the revision of the Mozilla Public License** at
  Mozilla; boards of Creative Commons and others; past OSI and GNOME boards. Writes on open
  source and AI.
- **Richard Fontana** -- Red Hat; long-standing license-policy voice; associated with
  popularizing **"inbound=outbound"** `[U on coinage]`. Current role `[U]`.
- **Pamela Chestek** -- **Chestek Legal**; trademark and copyright, counsel to open-source
  nonprofits. **The trademark expert** in this space -- relevant to §3.7. **Not** OSI chair.
- **Catharina Maracke** -- OSI director *and* FLA-2.0 co-author, unusual for spanning both the
  CLA and OSD worlds. **Carlo Piana** -- OSI director. **Matija Šuklje** -- FLA-2.0 co-author.
- **Simon Phipps** -- former OSI president; began OSI's transition to membership governance
  (2012).

**Canonical documents to read directly**
FSF **GPL FAQ**; the **OSD**; the **SPDX License List** (spdx.org/licenses) and expression
spec; **Apache's own GPL-compatibility page** (apache.org/licenses/GPL-compatibility.html)
`[V]`; the **REUSE Specification**; **Mozilla's MPL-2.0 FAQ** `[V]`; the **OFL FAQ**
(openfontlicense.org, 1.1-update7) `[V]`.

---
