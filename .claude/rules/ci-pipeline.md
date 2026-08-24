---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# CI / Build / Release Pipeline

A reference for evaluating CI / build / release pipelines from a security, reliability, and supply-chain integrity lens during review. Used by the `ci-pipeline` subagent.

The scope is **the pipeline itself** -- GitHub Actions / GitLab CI / Buildkite / Jenkins / CircleCI / Azure Pipelines / Tekton workflow files, Dockerfile design, release engineering configuration, deployment gating, artifact signing / provenance, branch protection rules, runner / agent security, caching strategy, build determinism, secrets management in workflows, OIDC trust to clouds.

Scoped against:
- **`security`**: owns code-level supply-chain concerns (lockfiles committed, dependency vulnerabilities, postinstall scripts as a concept, secrets-in-source code). This agent owns the operational pipeline-level concerns (action pinning by SHA, secrets in CI logs, workflow-level least-privilege, OIDC, branch protection, signed artifacts).
- **`distsys-runtime`**: owns the runtime behavior of canary / blue-green / rolling deploys (tail latency during rollout, partial-failure semantics, traffic-splitting math). This agent owns the operational pipeline mechanism for those strategies (deploy gates, rollback paths, smoke tests, environment promotion).
- **`performance`**: owns runtime production performance. Pipeline build duration matters but is secondary to correctness and security.

The core thesis: **the pipeline is the most privileged code in your system.** It builds the artifacts that run in production; it holds the secrets that access production; it decides what does and doesn't go out. Review CI/CD configuration with the rigor of production infrastructure-as-code, not the rigor of a Makefile.

The operational thesis: **pin everything; verify everything.** Pin all references to immutable identifiers (full SHAs for actions, digests for images, hashes for downloads); verify signatures and attestations at every consumption point. Defense in depth: branch protection, required reviews, signed commits, scoped tokens, OIDC trust, ephemeral runners, image signing, deploy gates, post-deploy smoke tests, canary rollouts. No single control catches everything.

---

## Universal principles

### Pin to immutable references

The single highest-leverage habit, articulated in many incident post-mortems:

- **GitHub Actions**: `uses: actions/checkout@<full-40-char-SHA>`, not `@v4` (tag is mutable; can be force-pushed). The 2025 `tj-actions/changed-files` compromise was a force-pushed tag.
- **Container images**: `image: registry/foo@sha256:<digest>`, not `image: registry/foo:v1` (registry tags are mutable).
- **Downloaded scripts**: never `curl | bash` from a URL whose target can change without notice. Vendor the script or verify the SHA. The 2021 Codecov bash uploader compromise was the canonical case.
- **Package versions in build steps**: `apt-get install foo=1.2.3-1`, `pip install foo==1.2.3`, not unpinned versions. Lockfile-driven installs (`npm ci`, `bundle install --frozen`, `pip install -r requirements.txt --require-hashes`) are the right pattern.

The marginal cost is small (a renovate-bot config can keep SHAs current); the marginal benefit (defense against tag mutation, registry compromise, transitive supply-chain attacks) is large.

### Least-privilege secrets and tokens

- **GitHub Actions `permissions:` block** -- every workflow should specify minimum required scopes. Default-write is a legacy footgun; the agent should flag missing `permissions:` on workflows that don't have one. `permissions: read-all` is a safer org-level default; `permissions: write-all` is almost never correct at the workflow level.
- **OIDC over long-lived credentials.** AWS IAM Roles for GitHub Actions, GCP Workload Identity Federation, Azure federated credentials all replace static cloud keys with short-lived OIDC-issued tokens. The OIDC `sub` claim should be scoped (`repo:org/repo:ref:refs/heads/main` or `repo:org/repo:environment:production`), not wildcarded (`repo:org/repo:*`).
- **Environment secrets**, not repository secrets, for production. GitHub Environments add a separate scope; production secrets only available to workflows targeting the `production` environment.
- **Vault / secret-manager as the source of truth.** CI fetches just-in-time via OIDC; doesn't hold static creds. Secret rotation cadence documented; rotation tested.

**Flag**: workflows with no `permissions:` block; OIDC trust with wildcard `sub`; long-lived AWS/GCP/Azure keys stored as CI secrets when OIDC is available; secrets stored at repo scope when environment scope is appropriate.

### The `pull_request_target` trap

`pull_request_target` runs with the **base** repository's permissions and secrets, even on PRs from forks. Workflows triggered by `pull_request_target` that check out the PR's head code execute attacker-controlled code with privileged credentials. This is the canonical GitHub Actions security trap.

**Flag**: any `pull_request_target` workflow that runs `actions/checkout` with `ref: ${{ github.event.pull_request.head.sha }}` and then runs any user-supplied build / test step. The pattern is correct only when the workflow is read-only (labeling, comments) and does not execute PR code.

### Build determinism and hermeticity

Hermetic builds: inputs are explicit, no network access during build, same inputs produce same outputs. Hermeticity buys cache-friendliness, audibility, and detectability of supply-chain tampering.

- **Tools**: Bazel, Buck2, Pants, Nix, Guix.
- **Dockerfile**: `--frozen-lockfile`, pinned package versions, `SOURCE_DATE_EPOCH` for timestamps, multi-stage builds with deterministic base images by digest.
- **Reproducible Builds project** (Debian, NixOS): bit-for-bit deterministic outputs.

**Flag**: `apt-get update && apt-get install <pkg>` without `=<version>` (latest pulls); Dockerfile that fetches via `curl https://...` without checksum verification; build steps that consult external APIs (timestamps, geolocation, network) for build-affecting data; image tags by `latest` or moving tags in `Dockerfile FROM` lines.

### Signed artifacts and provenance

The post-SolarWinds shift. Build outputs should carry attestation of *how they were built*:

- **Sigstore / Cosign** for container signing (keyless via OIDC; transparency log via Rekor).
- **SLSA attestations** for build provenance. SLSA v1.0 (April 2023) has Build levels 1-3 and tracks for Source, Dependencies, Verification. GitHub-hosted runners, GitLab SaaS, Google Cloud Build all produce SLSA L2-L3 attestations automatically.
- **SBOMs** (SPDX 3.0, CycloneDX 1.6) listing dependencies. Required for US federal procurement (EO 14028).
- **npm provenance** via GitHub Actions OIDC (rolled out March 2023). PyPI provenance rolling out.

**Flag**: production artifacts with no signature; signatures generated but never verified at consume time; missing SBOM for production artifacts; SLSA attestation absent on a release pipeline that could trivially produce it.

### Branch protection and gating

- **Required status checks**: tests pass before merge. Check the names match current workflow job names (silent no-op if they don't).
- **Required reviewers** on main / production branches; CODEOWNERS covering sensitive paths (workflows, infrastructure, secrets, deployment config).
- **Required signed commits** on tags and release branches, not just default branch.
- **Linear history** (squash or rebase); restricted push to main.
- **"Administrators can bypass"** is a footgun; flag if enabled on production-affecting branches.

**Flag**: branch protection disabled; "admins can bypass" on production branches; CODEOWNERS that doesn't cover `.github/workflows/` or `terraform/` or equivalent; auto-merge enabled with no human-review requirement; required checks that reference renamed jobs.

### Deploy safety

- **Rollback path**: every production deploy has a documented automated rollback path.
- **Canary deploys**: success criteria automated; automatic rollback on failure.
- **Database migrations**: expand/contract (additive, then cleanup across releases), never bundled schema-breaking with code that depends on the new schema. See `~/.claude/rules/system-design-patterns.md` for the expand-contract pattern.
- **Smoke tests in production** post-deploy; deploy success defined as "deployed service is healthy," not "deploy job exited 0."
- **Deploys serialize.** Concurrency control on production deploy workflows (`concurrency:` group `production-deploy`, cancel-in-progress: false).

**Flag**: production deploys with no rollback documentation; database migrations bundled with code depending on the new schema; canary deploys with no automated success criteria; deploy success measured at job exit, not service health; missing `concurrency:` on production-deploy workflows.

### Runner / agent hygiene

- **Ephemeral runners**, not persistent. Each job in fresh isolated environment.
- **Self-hosted runners** never on public repos (anyone with a PR can run code on your runner).
- **Runner network isolation** from production secrets / KMS / databases.
- **No cached credentials** accessible across jobs on the same runner.

**Flag**: self-hosted runner used on a public repository; persistent runner pool without per-job isolation; runner host with reach into production network beyond what the workflow strictly needs.

---

## Workflow design

### Trigger choice

- **`push`**: runs on push to listed branches. Use for main-branch CI, release tags.
- **`pull_request`**: runs in PR context. Forked PRs don't get repo secrets (correct default). Use for PR validation.
- **`pull_request_target`**: runs in *base* repo context with secrets. **Dangerous; see the trap above.** Only for read-only label / comment workflows that don't check out PR head.
- **`schedule`** / `cron`: scheduled runs. Beware of API drift if hitting external services.
- **`workflow_dispatch`**: manual trigger. Useful for production deploys with explicit operator action.
- **`workflow_run`**: triggered by another workflow's completion. Often used for "publish on tests pass." Beware: needs explicit commit / branch validation if downloading artifacts.
- **`repository_dispatch`**: external trigger via API. Authenticate the trigger source.

### Concurrency control

```yaml
concurrency:
  group: production-deploy
  cancel-in-progress: false
```

Production deploys serialize. CI builds on the same branch usually cancel-in-progress.

**Flag**: deploy workflows without `concurrency:`; PR CI without `cancel-in-progress: true` (wastes runners on stale commits).

### Reusable workflows and composite actions

The DRY pattern: an organization defines a "blessed" CI/CD pipeline (security-reviewed, tested), product repos call it. Caller passes inputs; the reusable workflow's permissions are explicit.

**Flag**: org with many similar workflows that should be a reusable; reusable workflow that doesn't pin its callers' token permissions; composite action that conceals supply-chain dependencies.

---

## Container build hygiene

### Multi-stage builds

Build in one stage; copy artifacts to a minimal runtime stage. Smaller, more auditable, fewer attack surfaces.

### Base image selection

- **Distroless** (Google): minimal, no shell, no package manager. Best for production runtime.
- **Alpine**: small, has shell, musl libc (sometimes causes problems with prebuilt binaries).
- **Ubuntu / Debian slim**: largest but most compatible.

Pin by digest, not by tag. `FROM ubuntu:24.04` allows the registry to change what `24.04` points to; `FROM ubuntu@sha256:...` doesn't.

### Image hygiene

- **`.dockerignore`** to exclude secrets, build artifacts, `.git`, `node_modules` that shouldn't ship.
- **Non-root user** in the final stage (`USER` directive).
- **Minimize layers** for cache efficiency.
- **`ARG` vs `ENV` vs `COPY`** ordering matters for layer caching.
- **Image scanning** before push: Trivy, Grype, Snyk.
- **Image signing** at push: Cosign.

**Flag**: containers running as root in production; `latest` tags; `Dockerfile FROM` by tag in production-bound images; missing `.dockerignore` (especially without `node_modules` / `.git` exclusion); image scan not in CI.

---

## Caching strategy

### Cache key design

Key must include all inputs that affect the output. A stable key with evolving inputs produces stale outputs that can persist for days.

```yaml
# Wrong: key never changes
key: build-cache-v1

# Right: key changes when inputs change
key: build-${{ runner.os }}-${{ hashFiles('**/package-lock.json') }}
```

### Cache scope and trust

- PR caches should not pollute main-branch caches (cache poisoning risk).
- Trusted caches on protected branches; untrusted on PRs.
- Cache observability: hit rate, size, age.

**Flag**: cache keys without source-input hashing; cache used as a correctness dependency (build only succeeds with cache hit, not as an optimization); cache scope that crosses trust boundaries; no cache observability.

---

## Secret handling

### The fingerprints of leaked secrets

- `echo $TOKEN` in workflow scripts.
- `set -x` enabled in shell steps.
- `curl -v` showing auth headers.
- Build tools printing config (`docker info`, `kubectl config view`).
- Test failures dumping environment.

GitHub Actions auto-masks known secret values in logs but doesn't catch derivations (`echo "first half: ${TOKEN:0:5}"`).

**Flag**: any `echo $SECRET` or `set -x` in a step that handles secrets; long `set -e -x` blocks; debug output that includes env dumps; CI logs visible to non-authorized users on private repos.

### OIDC adoption

Replace long-lived static creds with OIDC issuance:

```yaml
permissions:
  id-token: write  # required for OIDC
  contents: read

steps:
  - uses: aws-actions/configure-aws-credentials@<sha>
    with:
      role-to-assume: arn:aws:iam::123456789012:role/GitHubActionsDeployRole
      aws-region: us-east-1
```

The trust policy on the AWS side restricts the `sub` claim to `repo:org/repo:environment:production` or similar.

**Flag**: AWS / GCP / Azure long-lived credentials stored as CI secrets when OIDC trust is available; OIDC trust with wildcard `sub`; OIDC trust without environment / branch scoping.

---

## Specific platforms

### GitHub Actions
**The high-yield findings**: missing `permissions:` block (most common single finding across real reviews); tag-pinned actions instead of SHA-pinned (second most common); `pull_request_target` misuse (third). Beyond those: `GITHUB_TOKEN` over-scoped, `secrets.*` accessible in untrusted contexts, OIDC trust wildcards, missing deploy environment protection.

### GitLab CI
**Pitfalls**: `protected: true` variables missing on production secrets; merge-request pipelines vs branch pipelines (which runs?); child pipelines with separate trust boundaries; runners shared across projects.

### CircleCI
**Pitfalls**: orbs as third-party code (pin by full version, not floating); context-scoped secrets vs project-scoped; the 2023 breach as a reminder that any third-party CI is one phishing attack away from secret rotation.

### Jenkins
**Pitfalls**: script approval bypassed; plugin sprawl (each plugin a supply-chain dependency); credentials plugin scope; pipeline-as-code via Jenkinsfile in repo vs script defined in UI (the former auditable, the latter not).

### Buildkite
**Pitfalls**: agent security on self-hosted infrastructure; pipeline-as-yaml committed but agent-resolved plugins are dynamic; plugins from arbitrary GitHub repos.

### Azure Pipelines
**Pitfalls**: task marketplace as supply chain; service connections with broad scope; templates shared across orgs.

---

## Anti-pattern catalog

The signal-to-noise list, organized by category:

### Trigger and permission
- `pull_request_target` checking out PR head code.
- `permissions: write-all` at workflow level, or missing `permissions:` with legacy default.
- `GITHUB_TOKEN` over-scoped.
- OIDC trust policy with wildcard `sub`.
- Production deploy workflow without `concurrency:` block.
- `workflow_run` downloading PR artifacts without source validation.

### Action pinning
- `uses: <action>@v3` on production-critical workflows (should be `@<full-SHA>`).
- `uses: <action>@main` (branch pin, mutable).
- Third-party action with no review, no SHA pin, no maintainer accountability.

### Secret handling
- Secrets logged via `echo`, `set -x`, or curl traces.
- Long-lived cloud credentials stored as secrets when OIDC is available.
- Production secrets accessible to non-production workflows.
- Secret rotation cadence undocumented or > 1 year.
- Credentials passed as positional CLI args (visible in `ps`).
- Credentials hardcoded in workflows.

### Build determinism
- Build talks to the internet for tooling (`curl | bash`, unpinned `apt-get install`).
- Outputs depend on `$(date)` / `$(hostname)` without `SOURCE_DATE_EPOCH`.
- Docker `latest` referenced in production deploys.
- Docker tag pinning (`image:v1`) in production instead of digest pinning.
- No SBOM for production artifacts.
- No build provenance / attestation.
- Artifacts unsigned; or signed but not verified at consume time.

### Caching
- Stable cache key with evolving inputs.
- Cache as correctness dependency (build fails without cache).
- Cache scope crosses trust boundaries.
- No cache observability.

### Deployment
- No automated rollback path.
- DB migrations bundled with code that depends on them.
- Schema-breaking changes in a single release without expand/contract.
- Production deploys from a developer laptop.
- Canary without success criteria or auto-rollback.
- Deploy success measured at job exit, not service health.

### Branch protection
- Branch protection disabled or admin-bypassable.
- Required checks reference renamed jobs (silent no-op).
- Auto-merge with no human review.
- CODEOWNERS missing for sensitive paths (workflows, infra, secrets).
- Signed commits required only on default branch.

### Runner / agent
- Self-hosted runner on public repo.
- Persistent runners (per-job ephemeral not used).
- Cached credentials across jobs on the same runner.
- Runner reachable into production network beyond strict need.

### Tests in CI
- Tests auto-retried until green with no flaky-test ticketing.
- Flaky tests quarantined indefinitely.
- Tests share state (DB, FS, env).
- No production smoke tests post-deploy.
- Integration test environments using production credentials.

### Release versioning
- Mutable version tags in production references.
- Semver violations (breaking in minor, features in patch).
- No CHANGELOG or generated and unreviewed.
- Release tag deleted and re-pushed.
- Production references branch names, not tags or digests.

### Observability
- Pipeline metrics not collected (build duration, success rate, deploy duration unknown).
- Pipeline alerts page on every flake.
- Real failures don't page (main broken for hours unnoticed).
- Deploy success defined as job exit, not service health.

---

## Historical incidents worth knowing

- **Codecov bash uploader (April 2021)**: `curl | bash` from a compromised source exfiltrated secrets from thousands of CI runs.
- **SolarWinds (December 2020)**: compromised *build pipeline* (not source) produced signed-malicious updates. The canonical SLSA-defeating attack: signing alone is insufficient if the builder is compromised.
- **Event-Stream (November 2018)**: maintainer-handoff attack; new maintainer added malicious dependency. Open-source supply chain doesn't require attacker sophistication; it requires patience.
- **Ledger Connect Kit (December 2023)**: phished maintainer's npm account; ~$600k stolen via malicious package version.
- **`tj-actions/changed-files` (March 2025)**: force-pushed tags hijacked a popular action; leaked secrets from thousands of repos. Reignited the "pin by SHA" debate.
- **Travis CI (September 2021)**: misconfiguration leaked `*_TOKEN` env vars to forked-PR builds for ~8 days. Mass migration to GitHub Actions followed.
- **Dependency Confusion (Alex Birsan, February 2021)**: registered internal-name packages on public registries; CI builds at Apple, Microsoft, PayPal, Tesla, Shopify pulled malicious versions.
- **CircleCI breach (January 2023)**: engineer's laptop session-token theft → admin console access → customer env-var exposure. Mass-rotation required within 72 hours.
- **XZ Utils backdoor (CVE-2024-3094, March 2024)**: two-year patient maintainer-trust attack; backdoor hidden in test fixtures executed by autoconf. Caught by a developer noticing 500ms latency.

The lessons cluster:
- Pin everything; verify everything.
- Build environment integrity matters as much as source integrity (SLSA).
- Maintainer-trust attacks are real and patient (XZ, Event-Stream, Ledger).
- Any third-party CI is one phishing away from secret rotation (CircleCI).
- Force-pushed tags are a vector (tj-actions). SHAs aren't.

---

## What is NOT a ci-pipeline finding

Signal-to-noise:

- **Source-level supply-chain concerns** (lockfile committed, vulnerable dependency, postinstall script as a concept, secret-in-source). Route to `security`.
- **Runtime behavior of deploy strategies** (tail latency during canary, traffic-splitting math, partial-failure semantics). Route to `distsys-runtime`.
- **Production performance** (response time, throughput in deployed code). Route to `performance`.
- **Code-level test quality** (assertion strength, test design, mock vs integration). Route to `test-coverage`.
- **Documentation quality of pipeline configs** (missing comments). Route to `documentation`.
- **Generic "use Bazel" / "rewrite in Nix"** suggestions. The agent's value is concrete pipeline-shape fixes, not paradigm advocacy.
- **Optimal build duration** as a primary concern. We flag pipeline correctness and security; raw speed is secondary.

---

## Severity calibration

Using `panel-contract.md`'s rubric; specific calibration:

- **blocker**: `pull_request_target` checking out and executing PR head code with secrets; long-lived production cloud credentials accessible to all branches; OIDC trust with wildcard `sub`; production deploys without rollback; signed-but-not-verified production artifacts in a context where unsigned upstream could ship.
- **major**: missing `permissions:` block (over-broad default); tag-pinned third-party actions on production-critical workflows; missing `concurrency:` on production deploys; canary without success criteria; database migration bundled with breaking code change; CODEOWNERS missing for workflows or infrastructure paths; secrets accessible to wrong environment scope; self-hosted runner on public repo.
- **minor**: cache key without input hash; non-distroless production image where it would fit; `latest` tag in non-production context; missing pipeline observability; image not scanned in CI.
- **nit**: workflow file style (job naming, env var conventions); minor Dockerfile cleanup.
- **insight**: structural -- "this org has 20 similar workflows; consider a reusable workflow"; "the pipeline has accreted three caching layers; consider unifying"; "SBOM generation would meet upcoming federal procurement requirements at low cost."

Confidence: high when the trigger is concrete (a specific workflow file, a specific permission, a specific tag); medium when reasoned (an org-level pattern that the agent infers from one file).

---

## Process for the ci-pipeline agent

1. **Identify the pipeline surface(s).** Which CI provider? Which workflow files? Dockerfiles? Infrastructure-as-code? Branch protection (via `.github/settings.yml` or API)?
2. **Read project pipeline conventions.** `docs/ci.md`, `docs/release.md`, `CONTRIBUTING.md` sections on deploy, any platform-specific guidance.
3. **Walk the triggers**: for each `on:` block, is the trigger appropriate? Is `pull_request_target` used safely?
4. **Walk the permissions**: every workflow should have an explicit `permissions:` block at minimum scope.
5. **Walk the action pins**: every third-party `uses:` should be SHA-pinned; trusted first-party actions can be tag-pinned with policy.
6. **Walk the secrets**: how are they obtained (OIDC vs static), where are they scoped, are they logged?
7. **Walk the build**: hermetic? Pinned? Reproducible? SBOM? Provenance? Signed?
8. **Walk the deploy**: rollback path? Canary criteria? Migrations? Smoke tests? Concurrency control?
9. **Walk the runners**: ephemeral? Network-isolated? Public-repo-safe?
10. **Walk the branch protection**: required reviews? CODEOWNERS coverage? Signed commits? Admin bypass disabled?
11. **Route to other lenses** where the finding's primary lens is theirs:
    - Code-level secret in source / lockfile issue / dependency CVE → `security`.
    - Canary traffic math / rolling-deploy partial failure → `distsys-runtime`.
    - Production performance during rollout → `performance`.
    - Pipeline doc quality → `documentation`.
12. **Stay read-only.**
