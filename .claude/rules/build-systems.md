---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Build Systems

A reference for reviewing the build graph: how targets are declared, how dependencies are expressed, and what makes a build correct, incremental, hermetic, reproducible, and cacheable. Used by the `build-systems` subagent.

Distinct from:
- **`ci-pipeline`**: CI workflows, runners, secrets, branch protection, artifact signing, deployment gating. **Three seams need naming rather than a clean handoff**: CI caching a *build* cache (the read/write trust boundary below is jointly owned); CI invoking the build tool (flag hygiene such as `--locked` and `npm ci` is ours, runner config is theirs); and cache poisoning via a mutable CI workflow, which sits exactly on the boundary.
- **`platform-release`**: what happens to the artifact after it is built.
- **`licensing-and-oss`**: dependency license obligations. We own resolution mechanics.
- **`security`**: general threat model. We own build-graph integrity.

The core thesis: **a build defect is a correctness defect that looks like a flake.** "It works after `clean`" is a diagnosis, not a fix -- it names an underspecified dependency. And once a cache is shared, an underspecified input stops being a local annoyance and becomes a mechanism for one machine to corrupt everyone else's outputs.

The operational priority: **ask what the cache key covers and what it silently omits.** Nearly every serious finding in this lens reduces to a declared-input set that is narrower than the actual-input set.

Verification markers: **[V]** verified against primary source, **[U]** unverified.

---

## Theory that makes findings principled

**"Build Systems à la Carte"** (Mokhov, Mitchell, and Peyton Jones, ICFP 2018; extended in JFP 2020) gives the vocabulary. Its taxonomy crosses a rebuilding strategy with a scheduling algorithm [V]:

| | Topological | Restarting | Suspending |
|---|---|---|---|
| **Dirty bit** | Make | Excel | — |
| **Verifying traces** | Ninja | — | Shake |
| **Constructive traces** | CloudBuild | Bazel | — |
| **Deep constructive traces** | Buck | — | Nix |

The load-bearing idea is the **applicative-versus-monadic split**: static dependencies can be extracted without doing any work, which is precisely why Make is restricted to them. Dynamic dependencies require running part of the build to learn the rest.

**Early cutoff requires two distinguishable pieces of state per key** [V] -- one for "I ran" and one for "the answer changed." This explains Make's absence from the table: modification time serves as both value and built-time, so a single field cannot distinguish them.

**Ninja's `restat` is modification-time equality, not content equality** [V]. It fires only when the tool itself declined to rewrite the file, so **it requires tool cooperation** where content hashing does not. Note also that `n2` uses names, modification times, and the command line rather than content hashes [V].

**Recursive Make Considered Harmful** (Miller, 1998) is usually cited with the wrong mechanism [V]. Miller does not blame recursion: *"It is not the recursion itself which is harmful, it is the crippled Makefiles which are used in the recursion."* The actual failure is that **the directed acyclic graph was artificially separated into incomplete pieces.**

**Glob correctness** generalizes cleanly: a glob is a dependency on the *set* of matching files, so **the directory listing must itself be a tracked key.** Bazel tracks it during package loading; CMake and Make do not, and CMake's own documentation says so.

**The failure is asymmetric, which is why glob bugs survive for months**: deleting a file is noisy and produces a link error, while **adding one is silent** -- the build simply omits it.

---

## Hermeticity and the cache-poisoning argument

The argument against undeclared inputs is not theoretical, and the vendors concede it.

**Bazel's own documentation** admits the action key is incomplete [V]: *"Bazel currently does not track tools outside a workspace. This can be a problem if, for example, an action uses a compiler from `/usr/bin/`."* Two developers with different compilers compute the same key and wrongly share results.

**Sandboxing does not save you** [V]: it *"doesn't hide the host environment in any way. Processes can freely access all files on the file system."*

**Gradle states the consequence directly** [V]: *"Missing task inputs can cause incorrect cache hits, where different results are treated as identical."*

**The asymmetry that justifies splitting read and write access**: reading a poisoned entry corrupts one machine, recoverably. **Writing one corrupts everyone who later reads it, including release artifacts, silently** -- because the outputs are plausible rather than broken. Developer machines are precisely where the input set is least likely to be complete, which is the argument for read-only developer access to a shared cache.

### CREEP: the 2026 escalation

**CVE-2025-36852** [V]. Nx deprecated all four of its self-hosted cache packages on 2025-05-21 with an unusually blunt statement: *"The flaw is in their design and cannot be patched."*

The mechanism: **the CI workflow definition is not part of the cache key.** A pull request with no source changes but a modified workflow hashes identically to the mainline and wins by writing first. Nx states other build systems are also vulnerable.

**Artifact signing does not mitigate this**, and the reasoning is worth carrying: CI holds the signing key, so a poisoned artifact signs validly. **Signing defends against a compromised cache server, not a compromised writer.**

**This sits on the seam with `ci-pipeline` and needs joint ownership rather than a handoff.**

---

## Reproducibility

The formal definition, verbatim [V]: *"A build is reproducible if given the same source code, build environment and build instructions, any party can recreate bit-by-bit identical copies of all specified artifacts."*

**Debian is at roughly 94% reproducible** across amd64 and arm64 in its development suites [V]. Timestamps are, per the project, the biggest single source of irreproducibility.

The nondeterminism catalog worth knowing: embedded timestamps and build paths, archive member ordering, locale and timezone leakage, parallelism-dependent output ordering, and **hash-iteration order** -- which is why `PYTHONHASHSEED` and `PERL_HASH_SEED` exist, and why **preferring an ordered map over a hash map matters especially in a build script** [V].

Compiler support: `-fdebug-prefix-map` across GCC and Clang 3.8+, with `-fmacro-prefix-map` and `-ffile-prefix-map` from GCC 8 and Clang 10 [V]. Note that **`BUILD_PATH_PREFIX_MAP` remains a draft** carrying a do-not-use-yet marker nine years on [V] -- it is not a standard.

**Two Rust beliefs were refuted by actually running builds rather than reasoning** [V]:
- **`codegen-units > 1` does not break determinism.** Bit-identical output across repeated clean builds at 16 units.
- **Nondeterminism requires incremental compilation *and* debug info together**; either alone reproduces. The practical consequence: **release builds are reproducible by default and development builds are not.**

**The xz-utils backdoor (CVE-2024-3094) is a build-graph incident, not a CI one**, and belongs here [V]. Every layer lived in the build graph: the release tarball differed from the repository because an autotools-generated file shipped only in releases; `configure` executed attacker-controlled data extracted from a **test fixture**; and the payload edited a generated makefile. The lesson is that **the artifact you audit and the artifact you build must be the same artifact.**

---

## Per-toolchain

### Gradle

**Configuration cache has been stable since 8.1 but remains off by default in 9.x, becoming the default in Gradle 10** [V]. Isolated Projects is incubating; contrary to common belief, the cross-project configuration blocks are **not banned outright** -- only mutable-state access through them is [V].

**Archive reproducibility defaults inverted in Gradle 9** [V]: file timestamps are no longer preserved and file order is reproducible by default.

**Two silent killers in the annotation system** [V]:
- **Annotations on setters or bare fields are ignored.** The task then has an incomplete input set with no error.
- **Any validation warning makes a task never up-to-date, uncacheable, non-parallel, and non-incremental.** One warning silently disables four optimizations.

**The default path sensitivity is absolute**, which is the single largest cause of cross-machine cache misses [V].

**A run-script or custom task with no declared inputs and outputs defeats up-to-date checking entirely** and runs on every build.

### Bazel

**Bazel 9 removed the legacy workspace mechanism entirely** [V] -- the enable flags are now no-ops. Anyone still planning a migration is planning against a version that no longer exists.

`--incompatible_strict_action_env` now defaults to true [V]. Note that **layering check is clang-only on Unix and macOS in stock toolchains** [V], which is a real limit on C++ strict-dependency enforcement.

Buck2 is very active but **ships every release marked prerelease**, and **no large-scale adoption outside its originating company could be corroborated** [V]. Pants and Please are maintained by very small teams [V]. Chromium's build tooling lineage resolved to a current third-generation tool, with the two predecessors dead and fading respectively [V].

### CMake, Make, Ninja

`add_custom_target` is **always considered out of date** [V] -- the mechanism behind most missing-incrementality complaints.

**`file(GLOB)` for sources does not trigger reconfiguration when a file is added**, so the build silently omits it. CMake's documentation says so; the configure-time convenience is paid for at debugging time.

The usage-requirement model is the core of modern CMake: `target_link_libraries` with public, private, and interface scoping propagates include directories, definitions, and flags transitively. Directory-level commands and global flag variables bypass that model and are the classic anti-pattern.

**macOS still ships a Make from 2006** [V], which matters for any project assuming modern features.

### Rust and Cargo

**The four sources of compiler flags are mutually exclusive** [V]: setting the environment variable **silently discards the entire configuration-file block**. This is a top-tier CI footgun, since the local and CI builds then compile different code with no error.

Passing an explicit target triple, even the host's, is the documented way to stop those flags leaking into build scripts [V].

**Feature unification follows the resolved dependency graph, not the package selection** [V] -- so building a workspace subset does not necessarily unify everything, contrary to common belief.

**Lockfile guidance is now needs-based rather than library-versus-binary** [V], and the scaffolding tool tracks it by default. `build.rs` remains the primary hermeticity hazard, since it can read the environment, the clock, and the network.

### JavaScript and TypeScript

The ecosystem moved substantially, and stale beliefs here are common [V]:
- **TypeScript 7.0 shipped in July 2026** as the native port, roughly an order of magnitude faster, with the binary still named `tsc`.
- **Turbopack is the default bundler in Next.js 16**; the opt-out flag is now the one that selects the old bundler.
- **Vite 8 ships a single default bundler** rather than the historical split between development and production tools.
- **Corepack was removed in Node 25.**
- **Lerna is actively maintained**, contrary to widespread belief.
- **webpack is not in maintenance mode** and receives daily commits.
- **Rspack explicitly does not target full webpack compatibility**, which matters when a plugin is load-bearing.

TypeScript project references with `tsc --build` and `.tsbuildinfo` are the incrementality mechanism; `isolatedModules` constrains what transpile-only tools can do safely.

### Xcode

**This section is a known gap.** Apple's developer documentation is fully JavaScript-rendered and returned no usable content to automated retrieval, so **run-script input and output file lists, build-setting precedence, and the associated environment variables could not be verified** [U].

**Do not assert Xcode specifics from this file.** The general principle holds and is safe to apply: **a script phase without declared inputs and outputs cannot participate in incrementality and will run every build.** Verify the exact key names against Apple's documentation or a local installation before recommending them.

### Others

Go's build cache and module-graph pruning are largely automatic; `go generate` is deliberately not run by `go build`, which is one pole of the checked-in-versus-generated debate [V].

MSBuild incrementality rests on inputs and outputs declared per target. Nix offers the strongest purity guarantees at a real adoption cost, and **flakes remain experimental after roughly seven years** with content-addressed derivations unstabilized [V] -- so the flagship correctness system has a known, unfixed incrementality weakness.

---

## Cross-cutting concerns

**Toolchain pinning** is the antidote to the works-on-my-machine defect class: pin compiler, SDK, and language-runtime versions in files that are checked in, and verify the wrapper where one exists.

**Lockfile policy** is a genuine cross-ecosystem disagreement -- **Cargo and NuGet currently give opposite advice** [V]. What is not disputed: CI must install from the lockfile with the flag that fails on drift rather than silently resolving.

**Monorepo builds** need affected-target detection, or every change rebuilds everything. The counter-failure is a graph so partitioned that a root-file change invalidates it all anyway.

**Measure before optimizing.** Every major tool has a profiler; reasoning about build performance without one reliably targets the wrong phase.

**Code generation** must either be checked in with staleness detection (regenerate and diff in CI) or generated at build time with a correctly declared dependency. The failure mode is the third option: checked in with no verification, drifting silently.

**Test caching interacts badly with flakiness**: a cache keyed on success means a flaky test that passes once poisons the result until inputs change.

---

## Anti-pattern catalog

### Correctness and incrementality
- Undeclared inputs anywhere -- system headers, tools resolved from the path, environment variables, the clock, the network.
- `file(GLOB)` for sources, which silently omits newly added files.
- A script phase or custom task with no declared inputs and outputs, defeating up-to-date checks.
- `add_custom_target` used where incrementality is expected.
- Gradle annotations placed on setters or bare fields, where they are ignored.
- Validation warnings tolerated, silently disabling caching, parallelism, and incrementality.
- Hand-written makefiles with no automatic header-dependency generation.
- Treating "it works after clean" as a resolution.

### Caching
- Shared cache with write access from developer machines.
- Cache key omitting the toolchain version.
- Absolute path sensitivity, guaranteeing cross-machine misses.
- The CI workflow definition excluded from the cache key.
- Relying on artifact signing to defend against a poisoned writer.
- Build correctness dependent on a cache hit.
- No cache-hit-rate observability, so degradation is invisible.

### Reproducibility
- Embedded timestamps, hostnames, or absolute build paths.
- Archive creation without normalized ordering and metadata.
- Iteration over a hash map determining output order, especially in a build script.
- Release artifacts built from a tarball that differs from the repository.

### Dependencies and toolchain
- CI resolving dependencies rather than installing from the lockfile.
- Compiler flags set through an environment variable, silently discarding the configuration-file block.
- Unpinned toolchain versions.
- Network access from a build script.
- Generated code checked in with no staleness verification.
- Test results cached on a suite with known flakiness.

### Stale assumptions
- Planning a migration away from a mechanism that has already been removed.
- Assuming a bundler or type checker's default is still what it was a year ago.
- Assuming a widely-declared-dead tool is unmaintained without checking.

---

## Schools of thought (preserve disagreement)

- **Hermeticity versus pragmatic native tooling.** Bazel's own framing for the strict case is that conventional builds give *"too much power to engineers and not enough power to the system"* [V]. The pragmatist rebuttal uses Bazel's own documented holes: untracked external tools and a sandbox that does not hide the host. Both are correct, which is why the honest question is what the shared-cache blast radius is rather than which philosophy wins.
- **Monorepo versus polyrepo.** The monorepo case rests on atomic cross-project change and a single version of each dependency; the cost is that everything needs affected-target detection to stay tractable.
- **Checked-in versus build-time code generation.** Checked-in is reviewable, diffable, and buildable without the generator, at the cost of drift. Build-time cannot drift, at the cost of a toolchain dependency for every contributor. Go's position that generation is deliberately not part of build is one considered pole.
- **Lockfile policy for libraries.** Genuinely unsettled, and two major ecosystems currently advise opposite defaults.
- **Nix adoption.** Strongest guarantees, real cost, and a flagship feature still experimental after seven years.
- **Whether remote caching is worth its discipline tax.** It pays only if the input sets are actually complete, and an incomplete one converts a speed feature into a correctness hazard.

---

## What is NOT a build-systems finding

- CI workflow structure, runners, secrets, branch protection, deployment gating. Route to `ci-pipeline`, noting the three shared seams.
- Signing, packaging, and distribution of the built artifact. Route to `platform-release`.
- Dependency license obligations. Route to `licensing-and-oss`.
- General supply-chain threat modeling. Route to `security`; we own build-graph integrity specifically.
- Runtime performance of the built software. Route to `performance`.
- Raw build duration as a primary concern. We flag correctness and cacheability; speed follows from those.
- Generic "adopt Bazel" or "rewrite in Nix" advocacy without a named defect.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: an undeclared input combined with shared cache write access, which lets one machine silently corrupt others' release artifacts; CI resolving dependencies rather than installing from the lockfile on a release path; a build whose correctness depends on a cache hit.
- **major**: `file(GLOB)` for sources; a script phase with no declared inputs and outputs; compiler flags set by environment variable, discarding the configuration block; unpinned toolchain; generated code checked in with no staleness check; test caching over a known-flaky suite; validation warnings tolerated.
- **minor**: absolute path sensitivity; missing cache observability; archive metadata not normalized; unnecessary rebuild scope.
- **nit**: target naming; build-file organization.
- **insight**: structural -- "every change invalidates the graph at the root, so incremental builds never help here"; "this build reads three tools from the path, so the cache key describes a machine rather than a build"; "the release artifact is produced from a tarball that is not the repository, which is exactly the xz shape."

Confidence: high when the trigger is a concrete declaration, flag, or missing annotation; medium when reasoned from build shape. **Verify version-dependent claims**, since several toolchains changed defaults within the last year.

---

## Process for the build-systems agent

1. **Identify the build systems in play** and their versions. Defaults moved recently across several ecosystems.
2. **Ask what the cache key covers**, and what it omits. Most findings reduce to this.
3. **Walk declared inputs and outputs** for every custom task or script phase.
4. **Walk hermeticity**: tools from the path, environment variables, network access, absolute paths, the clock.
5. **Walk dependency resolution**: lockfile present, CI installing rather than resolving, toolchain pinned.
6. **Walk the cache trust boundary**: who writes, and does the key include the workflow and the toolchain?
7. **Walk reproducibility** where it matters: timestamps, paths, ordering, and whether the released artifact matches the repository.
8. **Walk code generation**: checked in with verification, or generated with declared dependencies.
9. **Check test caching** against known flakiness.
10. **Route to other lenses**, naming the three shared seams with `ci-pipeline` rather than handing off silently.
11. **Mark Xcode specifics unverified** unless confirmed against current documentation.
12. **Stay read-only.**
