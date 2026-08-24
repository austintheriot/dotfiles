---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# People, Management, Organization, and Culture

A reference for management, leadership, team organization, company culture, work distribution, communication patterns, hiring, performance, and the org dynamics surrounding engineering work. Used by the `people-and-org` advisor subagent and the `/comms-and-team` skill.

This is an **advisor** lens, not a code-review lens. The assistant invokes this when the user asks people / management / org / culture / comms questions: "how do I run my 1:1s," "this engineer is struggling," "we need to reorg," "how do I give difficult feedback," "we have a culture problem," "how do I run a postmortem," "how do I write this design-doc critique," "how do I have a hard conversation."

**Not auto-included in `/expert-review`.** People questions aren't code-shape questions.

The core thesis: **this domain is not short on platitudes, and the advisor's value comes from naming the specific framework, the canonical source, and the actual disagreement between schools of thought, then applying the right one to the situation at hand.**

The unifying observation: **the management craft has a relatively short historical canon (most defining books are 2014-2021) drawing heavily on a few earlier classics (Grove 1983, Peter 1969, Brooks 1975, Conway 1968).** Most modern advice is variation on a small core: outcomes over outputs, psychological safety as foundation, written communication scales, async beats sync for non-trivial work, hiring is the highest-leverage decision, feedback must be specific and frequent.

---

## Core principles (the through-line)

### Output equals throughput, not activity (Grove)

Andy Grove, *High Output Management* (1983). The manager's output is the output of the units under their supervision and influence. A manager who reads 200 emails and attends 12 meetings has high activity and possibly no output.

**Manager's leverage**: the multiplier between an hour of the manager's effort and the output it produces in the team. Coaching one report for an hour can affect their work for a quarter -- the highest-leverage activity.

**Task-relevant maturity (TRM)**: the variable that should set management style. A direct report new to a task needs structured, directive management; the same person mature at the task needs delegation and supportive coaching. Failing to vary style with TRM is the recurring management failure.

### Psychological safety is the foundation (Edmondson, Project Aristotle)

Amy Edmondson's research, validated by Google's Project Aristotle (Charles Duhigg, 2016): the highest-leverage team property is whether members can take interpersonal risks without fear. The five Aristotle factors in priority order:
1. **Psychological safety** -- can members speak up without humiliation?
2. **Dependability** -- can we count on each other?
3. **Structure / clarity** -- do we know our roles, goals, plans?
4. **Meaning** -- does this matter to us individually?
5. **Impact** -- does this matter to the organization?

Without safety, the other four don't compound.

### Outcomes over outputs

Same critique as the product-leadership canon, applied to people work. Performance reviews that measure activity ("shipped 17 PRs") miss the question ("did the system improve"). Hiring loops that measure interview output ("passed the technical bar") miss the outcome ("does this person thrive and deliver here").

### Conway's Law is empirical, not theoretical

Melvin Conway, 1968: "Organizations which design systems are constrained to produce designs which are copies of the communication structures of these organizations." The org chart shows up in the architecture diagram. The inverse Conway maneuver: design the team structure to produce the system you want.

### Hiring is the highest-leverage decision

Once you've hired someone, the cost of correcting a wrong hire is enormous (months of underperformance + months of replacement). The cost of saying no to a candidate who might have worked out is one candidate. The asymmetry favors slow, careful hiring.

---

## The canonical thought leaders

### Engineering management

**Andy Grove, *High Output Management* (1983)** -- still definitive. Output as throughput, manager's leverage, task-relevant maturity. The foundational text.

**Camille Fournier, *The Manager's Path* (2017)** -- the ladder-by-level book. Tech lead → engineering manager → director → VP → CTO. Each level's actual job is different; senior managers spending their time at junior-level work is the recurring failure.

**Will Larson, *An Elegant Puzzle* (2019), *Staff Engineer* (2021), *The Engineering Executive's Primer* (2024)** -- the working executive's reference. Org design as an explicit practice. The four staff-engineer archetypes: **Tech Lead** (technical direction for a team), **Architect** (cross-team technical strategy), **Solver** (deep difficult problems across the org), **Right Hand** (executive's force multiplier).

**Lara Hogan, *Resilient Management* (2019)** -- the BICEPS framework for what people need when they're upset: **B**elonging / **I**mprovement / **C**hoice / **E**quality / **P**redictability / **S**ignificance. When someone reacts strongly, identify which need is threatened.

**Julie Zhuo, *The Making of a Manager* (2019)** -- the new-manager guide; the "manager mindset" transition from IC.

**Michael Lopp ("rands"), *Managing Humans* (3rd ed 2016)** -- engineering management with personality.

### Feedback and hard conversations

**Kim Scott, *Radical Candor* (2017)** -- the 2x2: Care Personally × Challenge Directly. Ruinous Empathy (care, no challenge), Obnoxious Aggression (challenge, no care), Manipulative Insincerity (neither), Radical Candor (both).

**Stone, Patton, Heen, *Difficult Conversations* (2nd ed 2010)** -- the Harvard Negotiation Project. Three layers: **What happened** / **Feelings** / **Identity**. Most difficult conversations are mis-categorized as the "what happened" layer when they're actually about feelings or identity.

**Heen, Stone, *Thanks for the Feedback* (2014)** -- the receiver's discipline. Three triggers that make feedback hard to receive: **truth** (we disagree), **relationship** (with this person), **identity** (it threatens our sense of self).

**Patterson et al., *Crucial Conversations* (3rd ed 2021)** -- VitalSmarts. The dialog-when-stakes-are-high framework. STATE model: Share facts, Tell your story, Ask for theirs, Talk tentatively, Encourage testing.

### Org / culture

**Patrick Lencioni, *The Five Dysfunctions of a Team* (2002), *The Advantage* (2012)** -- the trust pyramid: **Absence of trust** → **Fear of conflict** → **Lack of commitment** → **Avoidance of accountability** → **Inattention to results**.

**Patty McCord, *Powerful* (2017)** -- Netflix CHRO. "Freedom and Responsibility." The keeper test: would you fight to keep this person if they tried to leave?

**Reed Hastings & Erin Meyer, *No Rules Rules* (2020)** -- the Netflix culture book.

**Ed Catmull, *Creativity, Inc.* (2014)** -- Pixar. The Braintrust as a model of candor-with-purpose.

**Ben Horowitz, *The Hard Thing About Hard Things* (2014), *What You Do Is Who You Are* (2019)** -- the startup-leadership crucible.

**Daniel Coyle, *The Culture Code* (2018)** -- safety, vulnerability, purpose. Pixar / Navy SEALs / IDEO case studies.

**Amy Edmondson, *The Fearless Organization* (2018)** -- psychological safety, the Harvard research.

### Hiring

**Lou Adler, *Hire With Your Head* (2007)** -- performance-based hiring. Define the role in performance objectives, not skills.

**Geoff Smart, Randy Street, *Who* (2008)** -- topgrading. Structured interviews. "A-players" rhetoric (which the engineering-management community has largely walked back -- the framing rewards arrogance).

**Laszlo Bock, *Work Rules!* (2015)** -- former Google SVP People Operations. Structured interviews; hiring committees > hiring manager alone.

### Communication and writing

**Jeff Bezos / Amazon** -- the six-pager memo. PowerPoint banned for serious decisions. Working backwards from a press release. The narrative culture.

**William Zinsser, *On Writing Well* (30th anniversary ed 2006)** -- clear writing, period.

**Cal Newport, *Deep Work* (2016), *A World Without Email* (2021)** -- focus as a contested resource; async-first defense.

### Org design / team topology

**Manuel Pais, Matthew Skelton, *Team Topologies* (2019)** -- the taxonomy: **stream-aligned** (deliver value to a stream of work), **enabling** (uplift other teams), **complicated-subsystem** (specialized component), **platform** (provide a service to other teams).

**Melvin Conway, "How Do Committees Invent?" (1968)** -- Conway's Law in its original form.

**Frederic Laloux, *Reinventing Organizations* (2014)** -- Teal organizations, self-management. Read but apply with skepticism.

### Async / remote

**Jason Fried, DHH, *Remote* (2013), *It Doesn't Have to Be Crazy at Work* (2018)** -- 37signals / Basecamp. Async-first; the calm-company thesis.

**GitLab handbook** (handbook.gitlab.com) -- the largest documented all-remote company. Worth reading even at companies that aren't remote-first.

### Coaching

**Michael Bungay Stanier, *The Coaching Habit* (2016)** -- the seven coaching questions:
1. **Kickstart**: "What's on your mind?"
2. **Awe (AWE)**: "And what else?"
3. **Focus**: "What's the real challenge here for you?"
4. **Foundation**: "What do you want?"
5. **Lazy**: "How can I help?"
6. **Strategic**: "If you're saying yes to this, what are you saying no to?"
7. **Learning**: "What was most useful for you?"

The discipline: ask, don't tell. Most managers default to advice; the coaching move is to surface the report's thinking.

---

## The laws

**Conway's Law (1968)**: organizations design systems that mirror their communication structures.

**Brooks's Law (1975)**: "Adding manpower to a late software project makes it later." Communication overhead scales N², the productive output linearly.

**Hyrum's Law** (applied to people-systems): with enough employees, every policy will be tested at its edge. "We trust people to use good judgment" becomes "someone made the wrong call."

**Goodhart's Law (Strathern, 1997)**: "When a measure becomes a target, it ceases to be a good measure." Critical for performance reviews, OKR design, incentive structures. Velocity-as-managerial-KPI is the classic.

**Parkinson's Law (1955)**: "Work expands so as to fill the time available for its completion." Also: "Expenditure rises to meet income."

**Pournelle's Iron Law of Bureaucracy**: "In any bureaucratic organization there will be two kinds of people: those who work to further the actual goals of the organization, and those who work for the organization itself. The second group will always control the organization."

**The Peter Principle (Peter, 1969)**: "In a hierarchy, every employee tends to rise to his level of incompetence." Promote-until-you-fail. The dual-ladder (IC and management) is a partial fix.

**Dunbar's Number (~150)**: the cognitive limit on stable social relationships. Implications: companies above ~150 need formal structure to replace informal coordination; companies above ~1500 need formal structure to replace department-to-department coordination.

**Allen Curve (Thomas Allen, 1977)**: communication frequency declines exponentially with physical distance. The 50-meter threshold: past it, casual collaboration evaporates. Modern implication: remote teams need deliberate communication design to overcome this.

**Hofstadter's Law**: "It always takes longer than you expect, even when you take into account Hofstadter's Law." Apply to project planning; double then add a unit.

**The Pareto Principle (80/20)**: 80% of effects from 20% of causes. Apply: 20% of meetings produce 80% of the value; 20% of features drive 80% of user value; 20% of customers produce 80% of revenue.

**The two-pizza team rule (Bezos)**: a team that can't be fed by two pizzas (~6-10 people) is too big.

**Ringelmann effect (1913)**: per-person productivity decreases as group size grows. Social loafing is real and measurable.

**Sayre's Law**: "In any dispute the intensity of feeling is inversely proportional to the value of the issues at stake." Why department fights are vicious.

**Cunningham's Law**: "The best way to get the right answer on the internet is not to ask a question; it's to post the wrong answer." Internal-discussion dynamic.

---

## Frameworks

### For 1:1s

**Cadence**: weekly, 30 minutes minimum, employee-driven agenda. Never canceled. (Camille Fournier is emphatic on this.)

**Agenda template** (Fournier-style):
- What's on your mind?
- What's blocked?
- What feedback do you have for me?
- Career / growth thread (rotating: weekly is too frequent for this; monthly is right).

**Lara Hogan's BICEPS** for when someone is upset:
- **Belonging**: do I belong here?
- **Improvement**: am I growing?
- **Choice**: do I have agency?
- **Equality**: am I being treated fairly?
- **Predictability**: do I know what's coming?
- **Significance**: does my work matter?

Identify which one is threatened; address that, not the surface complaint.

**Michael Bungay Stanier's seven questions** for coaching-flavored 1:1s: ask, don't tell.

### For feedback

**SBI** (Situation, Behavior, Impact): "In yesterday's review (S), you cut Sarah off three times (B), which made the meeting harder to follow and may have signaled her contribution wasn't valued (I)." Specific, observable, non-character-attacking.

**Radical Candor** (Scott): care personally + challenge directly. Both, simultaneously.

**Stone et al.'s three layers** for difficult feedback: what happened / feelings / identity. Most "what happened" disagreements are actually identity threats.

**Continuous feedback**, not annual. The modern shift away from one-shot annual reviews; weekly small feedback compounds.

### For team health

**Lencioni's pyramid**:
1. Trust (do we have each other's backs?)
2. Conflict (can we disagree productively?)
3. Commitment (are we aligned on direction?)
4. Accountability (do we hold each other to standards?)
5. Results (do we focus on outcomes?)

Each layer requires the one below.

**Project Aristotle**: psychological safety is the foundation; the other four (dependability, structure, meaning, impact) don't compound without it.

**Tuckman's stages**: forming → storming → norming → performing → adjourning. Storming is necessary and not avoidable; trying to skip it is a recurring failure.

### For org and team structure

**Team Topologies** (Skelton & Pais):
- **Stream-aligned**: deliver value to a stream of work (most teams).
- **Enabling**: uplift other teams' capabilities.
- **Complicated-subsystem**: specialized component (e.g., ML, video encoding).
- **Platform**: provide self-serve capabilities to other teams.

Interaction modes: **collaboration** (intense, short-term), **X-as-a-service** (well-defined, low-bandwidth), **facilitating** (coaching).

**Inverse Conway maneuver**: design the team structure to produce the system architecture you want.

### For meetings

**Amazon's six-pager**: written narrative replaces slides; first 20 minutes silent reading; discussion follows. For decisions and strategy.

**Working backwards from a press release**: for product / strategy ideation.

**DACI**: **D**river, **A**pprover, **C**ontributors, **I**nformed. Clarify who decides.

**RACI**: **R**esponsible, **A**ccountable, **C**onsulted, **I**nformed. Same idea.

**Lencioni's meeting categories** (*Death by Meeting*): daily check-in (5 min, standing), weekly tactical (1 hr, status), monthly strategic (4 hrs, 1-2 topics), quarterly off-site (1-2 days, big picture).

### For performance and promotions

**Calibration**: across-manager alignment on ratings; mitigates rater bias.

**Promotion packets**: written justifications of scope, impact, technical leadership.

**Engineering ladders**: levels with explicit expectations. The publicly-available ones (CircleCI, Patreon, Khan Academy, Rent the Runway, Slack, Carta) are useful references.

**Dual ladder**: IC and management as parallel paths, both leading to senior levels. Staff-plus engineering as a serious alternative to management.

### For postmortems and retrospectives

**Blameless postmortems** (John Allspaw, Etsy): the people involved did the best they could with the information they had. The system, not the individual, is the failure unit.

**"How Complex Systems Fail" (Richard Cook)**: 18 propositions about complex systems. Failure is the default; success is the surprise. Hindsight bias distorts post-incident analysis.

**Five whys (Toyota)**: superficial in software contexts (Allspaw's critique). Real root causes are usually plural.

**Action items with owners and dates** -- the only output that matters from a postmortem.

### For hiring

**Performance-based hiring (Adler)**: define the role in performance objectives, not skills lists.

**Structured interviews** (Bock / Google research): rubric-based, calibrated questions, blind review where possible. Reduces individual interviewer bias.

**Hiring committees** > hiring-manager-alone. Multiple perspectives reduce the "this candidate reminds me of myself" trap.

**Bar raisers** (Amazon): one panel member whose job is to ask "does this person raise our bar," not just "would they do the job."

### For decision-making

**Type 1 vs Type 2 decisions (Bezos)**:
- **Type 1**: irreversible / one-way doors. Hire/fire, public commitments, major architecture. Decide carefully.
- **Type 2**: reversible / two-way doors. Most decisions. Decide quickly; iterate.

The recurring management failure: treating Type 2 decisions as Type 1, slowing the org to a crawl.

**Disagree and commit (Bezos / Grove)**: once a decision is made, the people who disagreed support it publicly while continuing to flag concerns privately if relevant.

**"Strong opinions, weakly held"** (Saffo, often misattributed): hold opinions confidently but be willing to update on evidence.

---

## Specific topics

### The new-manager transition

The IC-to-manager transition is one of the most-failed in tech. The skill set is mostly different; "best engineer" doesn't map to "best engineering manager." Watkins's *The First 90 Days* applies to any transition; Fournier's chapter on the tech-lead-to-manager move is the engineering-specific reference.

The recurring failure: doing IC work as a manager because it's familiar and produces visible output. The manager's output is the team's output; doing IC work means not doing manager work.

### 1:1 cadence

Weekly 30-min, employee-driven agenda. Skip-levels (your manager's manager meeting your reports) quarterly. Skip-levels are signal: discrepancies between the report's account and the manager's account surface here.

### Continuous feedback vs annual reviews

The modern shift: annual ratings are mostly theater (predetermined, mis-calibrated, demotivating). Continuous feedback (weekly, in 1:1s, on specific situations using SBI) compounds.

Some orgs still need annual ratings for compensation / promotion calibration. The compromise: rating exists for the system, feedback happens continuously and never surprises the report at review time.

### Hard performance conversations

When performance is lagging, the conversation must be specific, early, and documented. The pattern:
1. **Name the gap** specifically (using SBI).
2. **Listen** to the report's account (often the work has context the manager missed).
3. **Agree on what change looks like** -- specific, observable.
4. **Set a timeline** -- weeks, not months.
5. **Check in frequently** -- the next 1:1, the one after.

PIPs (Performance Improvement Plans) are formal versions, often used as legal cover for already-made termination decisions. The discourse on this is real: PIPs work in some orgs, are termination notices in others.

### Layoffs and reorgs

Reorgs are the manager's reflex when the system isn't working. Sometimes the right move; often a substitute for harder work (replacing a specific person, clarifying a specific decision, killing a specific project).

Layoffs are uniquely hard: ethics (severance, communication), survivor's guilt, public optics. The handling matters as much as the decision.

### Cross-functional friction

The classic eng-product friction: PMs feel engineers don't ship; engineers feel PMs don't decide. Cagan's "product trio" (PM + designer + engineer co-owning outcomes) addresses some of this; org structure addresses more (Team Topologies).

The peer escalation pattern: "I'll talk to your manager" is nuclear; almost always the wrong first move. Direct peer conversation > manager involvement > skip-level.

### Postmortems

Action-items-with-owners is the only output that matters. The postmortem document is internal artifact; the cultural value is the discipline of blameless analysis. Bad postmortems blame the person who pressed the button; good postmortems ask why pressing the button was possible.

### Communication patterns

**Async > sync** for non-trivial work. A well-written document at 3pm produces decisions overnight; a meeting at 3pm produces a follow-up meeting next week.

**Written > verbal** for decisions. "I agreed in the meeting" is one person's memory; "I agreed in the Slack thread" is durable record.

**Group > 1:1** for broadcasts. Many 1:1s saying the same thing is a memo.

**1:1 > group** for sensitive feedback. Public feedback is humiliation.

### Documentation culture

The high-context written cultures (Amazon, Stripe, GitLab) over-invest in written docs; the gap between them and verbal-culture companies compounds over years. Design docs, ADRs, RFCs, runbooks, postmortems, brag docs, engineering-strategy docs -- the higher the org's depth of written thinking, the better its decisions scale.

### Coaching vs mentoring vs sponsoring vs managing

- **Coaching**: ask questions to surface the report's own thinking. Bungay Stanier.
- **Mentoring**: share your experience to inform their thinking. Hierarchy not required.
- **Sponsoring**: use your political capital to advance their career. The most-discussed gap for underrepresented engineers.
- **Managing**: setting direction, evaluating performance, making decisions about role and compensation.

A good manager does some of each; conflating them is the recurring failure.

### The staff-plus engineering track

Larson's *Staff Engineer* archetypes:
- **Tech Lead**: technical direction for a team. Common entry point.
- **Architect**: cross-team technical strategy.
- **Solver**: tackles the org's hardest problems. Often nomadic.
- **Right Hand**: executive's force multiplier; political dimension.

Each requires different skills. Senior IC tracks fail when companies promote into "staff engineer" without clarity on which archetype the role is.

---

## Anti-patterns

- **The Brilliant Jerk**: high-performing individual whose behavior tanks the team. Netflix's "no brilliant jerks." The discourse is contested -- some orgs tolerate this for star contributors; the long-term cost is real and underappreciated.
- **Bureaucracy capture**: Pournelle's Iron Law. The administrators win.
- **Hero culture**: rewarding firefighting over fire-prevention. Burns people out.
- **Cargo cult management**: copying surface practices (OKRs, Scrum) without the substance.
- **Reorg-as-strategy**: re-orging instead of doing the hard work of clarifying decisions / replacing specific people / killing specific projects.
- **Performance review theater**: ratings that don't differentiate; rankings predetermined.
- **Manager-as-blocker**: gatekeeping vs shielding.
- **The "smart but exhausting" colleague**: smart contributions wrapped in tone that costs more than they're worth.
- **Meeting culture without hygiene**: agendas without owners; recurring meetings that should be killed.
- **PIPs as cover for already-made decisions**: erodes trust in the process.
- **Underpromotion, overpromotion**: the calibration failure.
- **Feedback that's vague, late, or absent**: the most-common feedback failure.
- **"We hire for culture fit"**: often a euphemism for hiring people who look / think / act like the existing team. Culture *add* is the better framing.
- **Optimizing for HQ at the expense of remote workers**: hybrid-done-badly.
- **OKR theater**: form without function (Wodtke).
- **Status meetings**: should be async; aren't.
- **"Everyone is in every meeting"**: a sign of unclear decision-rights.

---

## Schools of thought (preserve disagreement)

- **Radical Candor vs Lencioni vs psychological safety**: all agree honest conversation matters; emphasize differently. Radical Candor frames it as a 2x2 personal skill. Lencioni frames it as a team-level pyramid (trust → conflict). Edmondson / Aristotle frame it as a team-level cultural property. The synthesis: each scale (individual / team / culture) has its own intervention.
- **Netflix Freedom and Responsibility vs traditional bureaucracy**: Netflix's model assumes high-performers + high context; works at Netflix's hiring bar; doesn't transplant easily.
- **Remote vs hybrid vs in-office**: contested post-COVID. The honest position: depends on the work, the team's tenure, the management capability. There is no universal answer.
- **Continuous feedback vs annual reviews**: continuous is better; some orgs need annual for calibration / compensation. The compromise model.
- **PIPs as legitimate development tool vs PIPs as legal cover**: real disagreement. Depends on the org's intent.
- **Bezos's six-pager vs slide culture**: real performance gap; the written-narrative cultures consistently outperform on decision quality. The slide cultures persist for political / hierarchical reasons.
- **Cagan vs Scrum / agile**: Cagan considers Scrum harmful to product (the "feature factory" critique).
- **Pournelle's pessimism vs Laloux's optimism on bureaucracy**: both have evidence. The pragmatic stance: bureaucracy is a system; design it deliberately or watch it design itself badly.

---

## Process for the people-and-org advisor agent

When invoked as an advisor:

1. **Diagnose the level**: is this a 1:1 question (individual), a team question (intra-team), an org question (cross-team / org design), or a culture question (org-wide values / behavior)? Different levels have different frameworks.
2. **Identify the situation**: feedback? Hiring? Reorg? Conflict? Hard conversation? Each has a canonical reference.
3. **Apply the right framework**: BICEPS for upset reports; SBI for feedback; Lencioni's pyramid for team health; Conway / Team Topologies for org design; Stone et al. for hard conversations; Adler / Bock for hiring; Grove for the manager's leverage question; Larson for staff-engineer archetypes.
4. **Cite the canon** when it grounds the advice. Don't perform erudition; cite specifically.
5. **Name the school of thought** when the advice depends on it. Radical Candor vs Lencioni vs Edmondson, Netflix vs traditional, remote vs hybrid -- the user's situation may live on a boundary.
6. **Apply the laws** where they fit (Conway, Brooks, Goodhart, Peter, Dunbar, Hofstadter, Pournelle, Sayre).
7. **Push past platitudes**. "Be a good manager" is noise. "In your 1:1 tomorrow, name the gap in specific behavior using SBI, listen for what's driving it (which BICEPS need is threatened), agree on what change looks like by week's end" is advice.
8. **Don't simply affirm.** Even on solid people-management thinking, find at least one assumption to challenge.

The user's stance per their global directive: "do not simply affirm. Provide counterpoints, test reasoning, offer alternative perspectives."
