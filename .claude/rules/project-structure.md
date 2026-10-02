---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-02
---

# Project Structure

A reference for reviewing how code is split into directories, modules, packages, crates, and repositories. Used by the `project-structure` subagent.

Distinct from:
- **`naming-conventions`**: the casing and wording of names, including file-name casing and case-only renames on case-insensitive filesystems. This file owns what a directory *means* (a `utils/` folder is a cohesion problem here). The neighbor owns how a file or directory name is *spelled*.
- **`build-systems`**: the build graph, cache keys, and incrementality. This file owns where module boundaries sit. The neighbor owns whether the build tool sees them correctly. **Shared seam**: TypeScript project references, Gradle subprojects, Cargo workspaces, and Android modules are both a structure decision and a build decision. Name the seam when a finding touches both.
- **`oo-architecture`**: hexagonal / clean / onion as dependency rules between classes. This file owns how those rules land on disk and whether anything enforces them.
- **`oo-domain-modeling`**: where bounded contexts are drawn. This file owns whether the folder and module layout follows the contexts once they are drawn.
- **`data-flow`**: who creates, owns, and consumes a runtime object. This file owns static import direction, not runtime ownership.
- **`readability`**: the reading order inside one file.

Verification markers: **[V]** verified against a primary source on the `last-verified` date, **[U]** found but not verified against a primary source, **[I]** inferred by the author.

---

## Thesis

**A directory tree is a claim about what changes together. It becomes a boundary only when something enforces it.**

Two facts carry most of the findings in this lens.

First, Parnas (1972) defined a module as "a responsibility assignment rather than a subprogram," characterized "by its knowledge of a design decision which it hides from all others" [V]. A folder that does not hide a decision is a label, not a module.

Second, Simon Brown, in the chapter he contributed to Robert Martin's *Clean Architecture* (ch. 34, "The Missing Chapter", 2017), showed that "if you make all types ... public, the packages are simply an organization mechanism (a grouping, like folders), rather than being used for encapsulation," and in that case "all four architectural approaches ... are exactly the same" [V]. Package-by-layer, package-by-feature, ports and adapters, and package-by-component collapse into one design when visibility does not differ between them. The layout argument is mostly an argument about *enforcement*, carried on in the vocabulary of folders.

**The operational question**: when the next ordinary change to this feature lands, how many top-level directories does the diff touch, and what stops a file in one unit from importing the internals of another?

### Empirical priority order

These bite most often, in this order. Triage in this order.

1. **Boundaries that exist only as folders.** A `features/` tree with no import rule, a monorepo with no visibility defaults, a Rails app with packwerk privacy turned off. The structure looks modular in a screenshot and is a single tangle at the import level. Shopify's packwerk retrospective is the field report: packages "were used as folders, not boundaries" [V].
2. **Dumping-ground shared modules.** `utils/`, `common/`, `shared/`, `helpers/`, `lib/misc`. They grow without limit because adding to them is free and removing from them needs a search of every caller.
3. **Wrong-axis split for the project's size.** By-type folders (`components/`, `reducers/`, `services/`) in an app with dozens of features, so one feature change touches five top-level folders. Or the opposite: an eight-segment feature-sliced hierarchy on a 3,000-line app.
4. **Artifact placement that misleads a tool.** Tests that import the source tree instead of the installed package (Python flat layout), Rust test helpers that compile as a test crate, Jest mocks in the wrong place, generated files that reviewers edit by hand.
5. **Barrel files.** Load-time and test-time cost in large JavaScript graphs, plus circular-import hazards. A live disagreement (see Schools of thought), but the performance data is not disputed.
6. **Moves that destroy history.** A combined move-and-edit commit that defeats rename detection, so `git blame` loses the lineage of the file.
7. **Structure that copies an org chart.** Conway's Law at the directory level. It is not wrong by default. It is wrong when the org changes and the tree does not.

---

## Volatile surface

`last-verified` (see frontmatter -- do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Next.js App Router file conventions (`_private`, `(group)`, `@slot`, `proxy.ts`) | Fast (each major) | nextjs.org/docs/app/getting-started/project-structure |
| Next.js `optimizePackageImports` status and default list | Fast | nextjs.org/docs/app/api-reference/config/next-config-js/optimizePackageImports |
| Feature-Sliced Design version, layer list, `@x` notation | Medium (yearly) | github.com/feature-sliced/documentation/releases |
| Angular file-naming and suffix defaults | Medium | angular.dev/style-guide |
| Nuxt directory structure (`app/` srcDir in v4) | Medium | nuxt.com/docs/4.x/directory-structure |
| TypeScript `baseUrl` / `paths` / `moduleResolution` deprecations | Medium | devblogs.microsoft.com/typescript |
| pytest default import mode | Slow | docs.pytest.org/en/stable/explanation/goodpractices.html |
| Packwerk feature set and maintenance | Medium | github.com/Shopify/packwerk/releases |
| Bulletproof React structure and barrel stance | Medium | github.com/alan2207/bulletproof-react/blob/master/docs/project-structure.md |
| Storybook story globs and file conventions | Medium | storybook.js.org/docs/writing-stories |
| Tuist module naming (µFeatures, now TMA) | Medium | tuist.dev docs |
| Xcode buildable folders / synchronized groups | Medium | Apple release notes, tuist.dev blog |
| Redux style guide priority levels | Slow | redux.js.org/style-guide |
| Maintenance of boundary tools (eslint-plugin-boundaries, dependency-cruiser, eslint-plugin-barrel-files, import-linter, Steiger) | Fast | each repo's commit recency |

Parnas, Conway, Martin's package principles, and the Brown visibility argument do not rot.

---

## Foundations

### Parnas: decompose by decisions likely to change

"On the Criteria To Be Used in Decomposing Systems into Modules" (CACM 15(12), December 1972) [V] compares two decompositions of one program.

- **Flowchart decomposition**: "make each major step in the processing a module." Parnas says this "was a useful abstraction for systems with on the order of 5,000-10,000 instructions."
- **Information-hiding decomposition**: each module hides one design decision. Things worth hiding include a data structure together with its access procedures ("not shared by many modules as is conventionally done"), control-block formats ("often proves extremely costly"), character codes, and processing order.
- The conclusion: "almost always incorrect to begin the decomposition ... on the basis of a flowchart ... begin with a list of difficult design decisions or design decisions which are likely to change ... modules will not correspond to steps in the processing."

Two consequences for directory review:

- **Folder-by-layer is a flowchart decomposition.** `controllers/ -> services/ -> repositories/` names the steps of a request, not the decisions each step hides [I]. Ousterhout's *A Philosophy of Software Design* calls the same mistake "temporal decomposition" [U].
- **Nesting depth is not modularity.** Parnas: "hierarchical structure and 'clean' decomposition are two desirable but independent properties" [V]. A deep tree can hide nothing. A flat list can hide a lot.

### Conway: structure follows communication

Melvin Conway, "How Do Committees Invent?" (*Datamation*, April 1968) [V]: "Organizations which design systems are constrained to produce designs which are copies of the communication structures of these organizations." The paper does not use the phrase "Conway's law." The name is usually credited to Fred Brooks [U].

Martin Fowler names three responses [V, martinfowler.com/bliki/ConwaysLaw.html]: ignore it, accept it, or apply the Inverse Conway Maneuver (change the team structure to get the architecture you want). Fowler adds a caveat: an org change alone does not repair a rigid architecture.

At directory scale: Google's monorepo makes the directory the unit of ownership ("Each and every directory has a set of owners") [V]. That ties the tree to teams on purpose. When teams reorganize, the tree must move, or ownership files drift from reality [I].

### Martin's package principles

From "Granularity" (*C++ Report*, 1996), *Agile Software Development: Principles, Patterns, and Practices* (2002), and *Clean Architecture* (2017) [U -- primary text not fetched, secondary at ootips.org].

Cohesion (what goes in a package):
- **REP**, Reuse/Release Equivalence: "The granule of reuse is the granule of release."
- **CCP**, Common Closure: "Classes that change together, belong together."
- **CRP**, Common Reuse: "Classes that aren't reused together should not be grouped together."

Coupling (how packages relate):
- **ADP**, Acyclic Dependencies: the package graph is a directed acyclic graph (DAG).
- **SDP**, Stable Dependencies: depend in the direction of stability.
- **SAP**, Stable Abstractions: a stable package is abstract.

Metrics: instability `I = Ce / (Ca + Ce)` and distance from the main sequence `D = |A + I - 1|` [U].

The three cohesion principles pull against each other. *Clean Architecture* draws them as a tension triangle [U]. A project early in life favors CCP (group by what changes together, so a change is local). A mature library favors REP and CRP (group by what consumers take together). **A layout that was right at year one can be wrong at year five without anyone making a mistake.** Findings must name which principle the current layout serves and which one the project now needs.

CCP is the most useful single test for application code: "when this feature changes, which files change with it?" If the answer spans the tree, the grouping fights CCP.

### Simon Brown: visibility is the architecture

Brown's chapter 34 of *Clean Architecture* [V via book text] is the best single source for this lens. Its argument in order:

1. Four options: package by layer, package by feature, ports and adapters, package by component.
2. Layered apps from different domains "look eerily similar: web, services, and repositories." The tree does not scream the domain.
3. Switching from layer to feature: "both are suboptimal."
4. **"If you make all types ... public, the packages are simply an organization mechanism ... all four architectural approaches ... are exactly the same."**
5. "I'd personally like to use the compiler to enforce my architecture." Discipline and code review fail when deadlines loom. Post-compile checking tools are "a little crude."
6. A component is "a grouping of related functionality behind a nice clean interface" (from the C4 model).
7. In .NET, `internal` needs "a separate assembly for every component." The Java Platform Module System (JPMS) and OSGi split "public" from "published."
8. One source tree per component is "idealistic." The closing line: "The devil is in the implementation details."

**How to apply**: before reviewing whether a layout is by-feature or by-layer, find out what the language and tooling let it *enforce*. Go `internal/`, Rust `pub(crate)`, Java package-private plus JPMS, .NET `internal` per assembly, TypeScript (nothing, without a lint rule or project references), Python (nothing, without import-linter), Ruby (nothing, without packwerk). In a language with no enforcement, a layout proposal without a lint rule is a naming proposal.

### Screaming Architecture, vertical slices, modular monolith

- **Robert Martin, "Screaming Architecture"** (2011-09-30) [V]: "When you look at the top level directory structure ... do they scream: Health Care System ... ?" And: "Frameworks are tools to be used, not architectures to be conformed to." The top level is the place to show the domain.
- **Jimmy Bogard, "Vertical Slice Architecture"** (2018-04-19) [V]: "Minimize coupling between slices, and maximize coupling in a slice." "New features only add code." Bogard says it needs a team that is skilled at refactoring, because duplication across slices is accepted at first and must be extracted later on evidence.
- **Kamil Grzybek, "Modular Monolith: A Primer"** (2019-12-02) [V]: a module "Must have everything necessary to provide desired functionality," and "Encapsulation is an inseparable element of modularity."
- **Philipp Hauer, "Package by Feature"** (2020-04-21) [V]: by feature gives discoverability and self-contained code. By layer means you "jump around from one package to another." Hauer cites Sandi Metz on the cost of needing to "understand everything in order to help with anything."

---

## The axes of a split

Every layout chooses a primary axis at each level. Name the axis per level before judging.

| Axis | Top-level folders look like | Serves | Fails when |
|---|---|---|---|
| **By type** (technical kind) | `components/`, `hooks/`, `reducers/`, `models/`, `controllers/` | Small apps. Framework conventions (Rails, early Redux). Discovery by kind. | Feature count grows. One change touches every top-level folder (CCP violation). |
| **By layer** | `web/`, `service/`, `repository/`, `domain/` | Enforcing one dependency direction across the whole app. | The layers are a flowchart (Parnas). Every domain looks the same (Brown). |
| **By feature** | `features/checkout/`, `features/search/` | Change locality (CCP). Deletion of a whole feature. Team ownership. | No rule stops features importing each other. "Feature" is undefined, so slices are inconsistent. |
| **By component** (Brown) | `orders/` exposing one interface, internals hidden | Encapsulation enforced by the compiler. | The language cannot hide the internals. |
| **By route / page** | `app/(shop)/cart/page.tsx` | Routing frameworks. Colocation of route-only code. | Logic reused across routes stays duplicated or moves to a dumping ground. |
| **By subdomain / bounded context** | `billing/`, `catalog/`, `identity/` | Large systems with distinct models. Phoenix contexts. | The contexts are guessed before the domain is understood. |

**Mixed axes are normal and correct.** The common good shape is by-feature (or by-context) at the top and by-type inside each feature: `features/todos/{todosSlice.ts, Todos.tsx, todosApi.ts}`. The common bad shape is by-type at the top with a by-feature tree duplicated inside each type: `components/todos/`, `reducers/todos/`, `api/todos/`. That second shape pays the cost of both and gets the benefit of neither [I].

### A measurable test: change spread

For a candidate layout, take the last 20 non-trivial commits that touched one feature and count distinct top-level directories per commit [I]. `git log --name-only` plus `cut -d/ -f1-2 | sort -u` gives the count. A median above three for ordinary feature work is a CCP signal worth reporting. This is evidence a reviewer can produce, which beats an appeal to a style.

---

## Redux and frontend organization

### Redux: the canon, with its priority levels

The Redux Style Guide [V, redux.js.org/style-guide] uses three priority levels:
- **A, Essential**: "prevent errors ... abide by them at all costs."
- **B, Strongly Recommended**: "violations should be rare and well-justified."
- **C, Recommended**: "an arbitrary choice can be made to ensure consistency."

**"Structure Files as Feature Folders with Single-File Logic" is Priority B.** The text: "most applications should structure files using a 'feature folder' approach ... the Redux logic for that feature should be written as a single 'slice' file, preferably using ... createSlice. (This is also known as the 'ducks' pattern) ... older Redux codebases often used a 'folder-by-type' approach ..."

The style guide's example tree:

```
/src
  index.tsx
  /app
    store.ts          # store setup
    rootReducer.ts    # optional
    App.tsx
  /common             # truly generic and reusable
    hooks/ components/ utils/
  /features
    /todos
      todosSlice.ts
      Todos.tsx
```

`/app` "depends on all the other folders." That sentence is the dependency rule: app composes features, features do not compose each other, and `/common` is "truly generic." Each clause is a review check.

Related priorities from the same guide [V]: "Use Redux Toolkit" is B. "Model Actions as Events, Not Setters" is B. `domain/eventName` action-type naming is C. Thunks and listeners for side effects is C. When citing the guide, cite the level. A B-level violation is a finding. A C-level difference is a consistency question, not a defect.

**Ducks** (Erik Rasmussen, ducks-modular-redux, repo created 2015-08-31) [V] predates Redux Toolkit. The rules: a duck MUST `export default` its reducer, MUST export its action creators, and MUST use action types of the form `npm-module-or-app/reducer/ACTION_TYPE`. It MAY export the action types as `UPPER_SNAKE` constants. The motivation: "95% of the time, it's only one reducer/actions pair." `createSlice` is ducks with the boilerplate generated, which is why the style guide equates them.

**RTK Query** [V, Redux Essentials part 7]: "only one createApi call" per app, and "one API slice per base URL." The tutorial puts it at `features/api/apiSlice.ts`. `injectEndpoints` lets each feature define its own endpoints in its own folder while sharing the single API slice. **A finding**: several `createApi` calls against one base URL create separate caches, separate middleware, and separate tag invalidation, so an invalidation in one feature does not refresh data fetched by another [I from the stated rule].

**Where feature folders break in Redux apps** [I]:
- Selectors that combine two slices. They belong to neither slice. Put them in the feature that consumes the combination, or in an `app/`-level selectors file. Do not put them in `common/`, which is reserved for code with no domain knowledge.
- `extraReducers` that listen to another slice's actions. This is the sanctioned cross-feature coupling in Redux (events, not setters), and it creates an import from one feature into another. Allow it as the documented exception, and watch that it is one-way.
- Normalized entities used everywhere (`users`). These are entity slices, not feature slices. Feature-Sliced Design gives them their own layer (see below). The Redux guide is silent.

### Dan Abramov and the React docs: emergent structure

- Abramov's react-file-structure.surge.sh [V]: "move files around until it feels right ... this is not a joke."
- The legacy React FAQ [V, legacy.reactjs.org/docs/faq-structure.html]: "React doesn't have opinions on how you put files into folders." It recommends "limiting yourself to a maximum of three or four nested folders," because deep relative imports are hard to write and to update after a move. "Don't spend more than five minutes on choosing a file structure." "If you feel completely stuck, start by keeping all files in a single folder."
- This page exists only in the legacy docs. react.dev has no equivalent [I].

### Kent C. Dodds: colocation

"Colocation" (2019-06-17) [V]: "Place code as close to where it's relevant as possible." Dodds argues against mirrored `__tests__` trees, and makes a deletion argument: a centralized utility outlives its last caller because nobody knows it is dead. Colocated code dies with the thing it served.

The deletion argument is the strongest practical case for colocation, and it is checkable: search the shared folders for exports with zero or one importer [I].

### Bulletproof React

[V, alan2207/bulletproof-react docs/project-structure.md]

- `src/` holds `app`, `assets`, `components`, `config`, `features`, `hooks`, `lib`, `stores`, `testing`, `types`, `utils`.
- Each feature folder may hold `api`, `assets`, `components`, `hooks`, `stores`, `types`, `utils`. Only the segments a feature needs.
- "It might not be a good idea to import across the features. Instead, compose different features at the application level."
- Direction: shared -> features -> app. Enforced with ESLint `import/no-restricted-paths`.
- **It reversed its barrel-file advice**: "it was recommended to use barrel files ... it can cause issues for Vite to do tree shaking ... recommended to import the files directly."

### Feature-Sliced Design (FSD)

[V, feature-sliced.design/docs/reference/layers, release history via GitHub]

Layers, top to bottom: `app`, `processes` (deprecated), `pages`, `widgets`, `features`, `entities`, `shared`. Each layer except `app` and `shared` is divided into **slices** (a business domain), and each slice into **segments** (`ui`, `api`, `model`, `lib`, `config`).

- The import rule: "A module (file) in a slice can only import other slices when they are located on layers strictly below." Slices on the same layer cannot import each other.
- `app` and `shared` have no slices. Their segments import each other freely.
- `processes`: "This layer has been deprecated ... moving its contents to features and app."
- Each slice exposes a **public API** (an index file): "a contract between a group of modules, like a slice, and the code that uses it."
- Cross-imports between entities use the `@x` notation, now standardized as "Public API for cross-imports."
- `shared` may hold route constants, API clients, and the logo. It may not hold business logic.
- The Steiger linter is described as "production-ready."

Versions: v2.0-beta 2021-05-17, v2.0.0 2023-10-01, **v2.1 2024-11-13, "Pages come first!"** There is no v3 as of `last-verified`.

**FSD v2.1 is a critique of FSD v2.0 by its own authors.** The release notes say entity-first and feature-first decomposition made "Code cohesion ... much worse," forced developers to "jump around several folders just to make changes to a single user flow," and made "Unused code ... harder to delete." Also: "Finding entities and features is still an advanced skill," and "Different developers have different understandings of these concepts." The v2.1 rule: code not reused elsewhere stays in its page or widget slice. **The most prescriptive frontend methodology converged toward colocation.** When a team cites FSD to justify extracting single-use code into `entities/` or `features/`, they are citing the version FSD itself walked back.

FSD also documents its own index-file problems [V]: circular imports, worse tree-shaking, dev-server slowdown. Its mitigations: one index per component in `shared/ui` and `shared/lib` (not one for all of `shared`), and never import your own slice through its own index.

### Framework-owned conventions

When the framework assigns meaning to a directory or file name, the layout is no longer free. A move can change behavior.

**Next.js App Router** [V, project-structure doc, v16.3.8, page updated 2026-07-21] **VOLATILE**:
- "Next.js is unopinionated" about project files.
- A route is public only when a `page.js` or `route.js` file exists in it, so "project files can be safely colocated" inside `app/`.
- `_folder` (private folder) opts the folder and all subfolders out of routing. One stated reason: "Avoiding potential naming conflicts with future Next.js file conventions." Use `%5F` for a literal leading underscore in a URL segment.
- `(group)` route groups are omitted from the URL and allow multiple root layouts.
- `@slot` defines parallel routes. `(.)`, `(..)`, `(...)` define intercepting routes.
- Three documented strategies: files outside `app/`, top-level folders inside `app/`, or split by feature or route.
- `proxy.ts` appears in the top-level files table. That it replaced `middleware.ts` in v16 is inferred [I]. Check before saying so.

**Nuxt 4** [V] **VOLATILE**: `app/` is the source directory, holding `components/`, `composables/`, `pages/`, `layouts/`. Also `server/`, `shared/`, `layers/`, `public/`. **Auto-imports are driven by directory name** [V]. Moving a composable out of `composables/` removes it from auto-import, and the symptom is a runtime "not defined" error in a file that never imported it explicitly [I].

**Angular** [V, angular.dev/style-guide, v22 era] **VOLATILE**:
- Angular v20 (May 2025) removed most file and class suffixes. `ng g c user` now generates `user.ts` with class `User`, not `user.component.ts` with `UserComponent`. The old behavior returns with the schematics option `"type": "component"` or `--type`.
- The current guide: "Avoid creating subdirectories based on the type of code ... avoid creating directories like components, directives, and services." Organize by feature area. `.spec.ts` files go "in the same directory as the code-under-test." Hyphenated file names. "Prefer focusing source files on a single concept."
- The LIFT principle (Locate, Identify, Flat, Try to be DRY) from the old angular.io guide does not appear in the current guide [U on its prior wording].
- **A finding**: a codebase that mixes pre-v20 suffixed files and post-v20 unsuffixed files has two conventions in one tree. Pick one and set the schematics default so `ng generate` keeps it.

**Storybook** [V, v10.6 docs]: stories "live alongside the component file" (`Button/Button.stories.tsx`). The story glob in `.storybook/main` determines discovery. A story placed outside the glob is not an error. It is invisible.

**Atomic design** (Brad Frost, 2013) [V]: "not a linear process, but rather a mental model," and "not rigid dogma." Frost does not prescribe `atoms/`, `molecules/`, `organisms/` as folders. Teams that use them as folders hit classification disputes (is a search bar a molecule or an organism?) and lose findability, because the folder encodes size, not purpose [U -- the criticism is from practitioner blog posts, not a primary source].

---

## Other ecosystems

### Go

- **golang-standards/project-layout is not a standard.** Russ Cox, issue #117 (2021-04-09) [V]: "the vast majority of packages ... do not put the importable packages in a pkg subdirectory ... just very complex, and Go repos tend to be much simpler." His follow-up (2021-04-28) [V] gives "the minimal standard layout": "Put a LICENSE file in your root; Put a go.mod file in your root; Put Go code in your repo, in the root or organized into a directory tree as you see fit. That's it." He adds that commands are not required to live in `cmd/` nor packages in `pkg/`, and "The importable golang.org/x repos break every one of these 'rules'."
- **go.dev "Organizing a Go module"** [V, go.dev/doc/modules/layout]: the basic layout is everything in the root. `internal/` "prevents other modules from depending on packages." `cmd/` is "very useful in a mixed repository." Server projects keep logic in `internal/`. **`pkg/` is not mentioned.**
- **`internal/` is compiler-enforced** [V, cmd/go docs]: an import is "disallowed if the importing code is outside the tree rooted at the parent of the 'internal' directory." This is the strongest built-in boundary in any mainstream language, and it is free.
- **Package names**: Sameer Ajmani, "Package names" (go.dev blog, 2015-02-04) [V]: "Packages named util, common, or misc provide clients with no sense of what the package contains." The worked example turns `util.NewStringSet` into `stringset.New`.
- **Tests**: `_test.go` in `package foo` is white-box and sees unexported names. `package foo_test` in the same directory is black-box and sees only the exported API [V]. Choosing between them is a structure decision: black-box tests catch an API that is unusable from outside.
- **Generated files** carry a line matching `^// Code generated .* DO NOT EDIT\.$` [V]. Linters and reviewers key on it.

### Rust

- **Module files** [V, Rust Reference, items/modules]: `util.rs` plus `util/config.rs`, or `util/mod.rs`. "Not allowed to have both." "Prior to rustc 1.30, using mod.rs files was the way ... It is encouraged to use the new naming convention ... avoids having many files named mod.rs." The Reference ties this to rustc 1.30, not to an edition. Saying "the 2018 edition introduced `foo.rs`" is a common imprecision.
- **Tests** [V, The Rust Book ch. 11.3]:
  - Unit tests live in each file as `#[cfg(test)] mod tests` and can test private items.
  - Integration tests live in `tests/`. Each file there compiles as its own crate and sees only the public API.
  - Shared helpers go in `tests/common/mod.rs`, **not** `tests/common.rs`. A `tests/common.rs` file compiles as its own test crate and appears in test output with zero tests.
  - A binary crate cannot be integration-tested. Keep logic in `lib.rs` with a thin `main.rs`.
- **Workspaces**: Aleksey Kladov (matklad), "Large Rust Workspaces" (2021-08-22) [V]:
  - A flat `crates/` directory works from 10k to 1M lines of code. rust-analyzer was about 200k lines in 32 crates, flat.
  - Use a virtual manifest at the root. A root package "pollutes the root with src/."
  - Crate name equals folder name.
  - `version = "0.0.0"` for internal crates that are never published.
  - `cargo xtask` for repository automation instead of shell scripts.
  - "Even comparatively large lists are easier to understand at a glance than even small trees."
- **ARCHITECTURE.md**: matklad (2021-02-06) [V]. For projects of 10k to 200k lines, a short root file with a bird's-eye overview and a codemap that answers "where's the thing that does X?" "Name important files, modules, and types. Do not directly link them" (links rot, names can be searched). State architectural invariants, including those expressed "as an absence of something" (module A never depends on module B). Revisit it a couple of times a year.
- **Visibility** is the enforcement tool: `pub(crate)`, `pub(super)`, and the crate boundary itself. Splitting a crate is the Rust equivalent of Go `internal/`, at the cost of compile units and an extra `Cargo.toml` (see `build-systems`).

### Python

- **src layout vs flat layout** [V, PyPA discussion]: the src layout prevents "accidental usage of the in-development copy" of the package, because the current working directory is first on `sys.path`. It also makes an editable install expose only importable files. The PyPA caveat: a command-line tool "can not be run directly from the source tree."
- **The failure the src layout prevents** [I from V]: in a flat layout, `pytest` from the repo root imports `mypkg/` from the working tree, not the installed wheel. Tests pass. The wheel is missing a data file or a subpackage because `pyproject.toml` did not include it. The bug ships.
- **pytest** [V, Good Integration Practices]: two layouts (tests outside the package, or tests inside it run with `--pyargs`). pytest strongly recommends the src layout. The default import mode is still `prepend`. In that mode "test files must have unique names" unless the test directories contain `__init__.py`. Symptom: two `tests/test_utils.py` files in different directories cause an import-file-mismatch error. New projects should use `--import-mode=importlib`.
- **Namespace packages** [V, PyPA guide]: "every distribution ... omits the `__init__.py` or uses a pkgutil-style `__init__.py`." Mixing styles breaks the namespace. A forgotten `__init__.py` in a regular package creates an implicit namespace package, and tools that discover packages (setuptools `find_packages`) skip it, so the subpackage is missing from the wheel [I].
- **import-linter** [V, v2.15] is the enforcement tool. Contract types: `forbidden`, `independence` ("no imports in any direction ... even indirectly"), `layers` (with containers and `exhaustive`), `acyclic_siblings`, and `protected` (an allow-list of importers).

### Java and Kotlin

- **Maven Standard Directory Layout** [V]: `src/main/java`, `src/main/resources`, `src/test/java`, `src/test/resources`, `src/it`, `src/site`. `target/` "Houses all output of the build." The stated benefit: "users familiar with one Maven project [can] immediately feel at home in another ... analogous to adopting a site-wide look-and-feel." Maven is by-type only at the top two levels. It says nothing about the package structure under `java/`, which is where by-feature or by-layer is decided.
- **Gradle multi-project** [V, Gradle 9.8 docs]: `include()` in `settings.gradle.kts`, "lower case hyphenation" for project names. Shared build logic goes in convention plugins under `buildSrc` or `build-logic`. Gradle advises against `allprojects` and `subprojects` because "build logic can be injected into a subproject which is not obvious" and causes "configuration-time coupling."
- **JPMS** splits "public" from "published" (Brown) [V via Brown]. Primary JPMS docs not fetched [U].
- **ArchUnit** writes architecture rules as unit tests ("classes in `..domain..` must not depend on `..web..`") [V repo active]. It is the Java answer to the enforcement gap.

### .NET

`internal` is per assembly, so component encapsulation needs one project per component (Brown) [V]. Solution and project layout conventions were not researched [gap].

### Elixir and Phoenix

Phoenix contexts [V, Phoenix 1.8 guides]: `lib/hello/catalog.ex` is the public API of the Catalog context. `lib/hello/catalog/product.ex` is a schema inside it. `lib/hello_web/` holds the web layer, which calls contexts and never reaches into their schemas directly. The guide is candid: it is "hard to draw lines or name its different contexts," so "pick a name that is clear and obvious to everyone." Contexts arrived in Phoenix 1.3 (2017) [U on date]. This is the cleanest mainstream example of folder-equals-bounded-context.

### Android

[V, developer.android.com/topic/modularization and /patterns]

- Module types: app, feature, data, common/core, test.
- A data module exposes only its repository.
- **Feature modules do not depend on each other.** The app module mediates through navigation, passing IDs, not objects.
- The api/impl split (`:database:api`, `:database:impl:room`): "Implementation changes don't recompile dependent modules."
- Prefer `implementation` over `api` dependencies. Prefer pure Kotlin or Java modules where Android APIs are not needed.
- Pitfalls named by the guide: too fine-grained ("increased build complexity and boilerplate"), too coarse ("yet another monolith"), and modularization "doesn't always make sense," with "size of the codebase" as the deciding factor.

### iOS: Tuist's modular architecture

[V, Tuist docs source via GitHub]

- The Modular Architecture (TMA), "previously known as µFeatures."
- Five targets per module: `Feature`, `FeatureInterface`, `FeatureTests`, `FeatureTesting` (mocks and fakes for other modules' tests), `FeatureExample` (a small app to run the feature alone).
- Modules depend only on another module's `Interface` target. This decouples implementations and speeds clean builds. It requires dependency injection at runtime to bind interfaces to implementations.
- Dynamic linking in development (for SwiftUI previews), static linking for release.
- Xcode 16 buildable folders and synchronized groups reduce `.pbxproj` merge conflicts [U, Tuist blog 2025-03-21 and secondary posts]. **VOLATILE**.

---

## Where each kind of artifact lives

The question for every non-source artifact: **which tool discovers it, and what happens when it is in the wrong place?** A misplaced artifact rarely fails loudly. It is silently ignored, silently included, or silently tested against the wrong copy.

### Tests

| Convention | Placement | Discovery | What it optimizes |
|---|---|---|---|
| Angular | `.spec.ts` beside the source [V] | Karma / Jest config | Colocation |
| Go | `_test.go` in the same directory [V] | `go test` | Colocation, white-box or black-box by package name |
| Rust unit | `#[cfg(test)] mod tests` in the file [V] | `cargo test` | Access to private items |
| Rust integration | `tests/*.rs`, helpers in `tests/common/mod.rs` [V] | `cargo test` | Public-API-only testing |
| Maven / Gradle | `src/test/java` mirroring packages [V] | Surefire | Same-package access, separate classpath |
| pytest + src layout | `tests/` outside `src/` [V] | rootdir + import mode | Testing the installed artifact |
| Jest / Vitest | `*.test.ts` beside source, or `__tests__/` | `testMatch` / `include` globs | Colocation |

The real distinction is not colocated versus separate. It is **what the test can see**: private internals (Rust unit, Go white-box, Java same-package), the public API (Rust `tests/`, Go `_test` package), or the *installed* artifact (pytest with src layout). Each answers a different question. A project needs to decide which questions it is asking, and the placement follows.

**Mirrored test trees** (`src/foo/bar.ts` + `test/foo/bar.test.ts`) cost an extra move per rename and drift when one tree is reorganized without the other. Dodds argues against them [V]. Maven and pytest-with-src require them for real reasons (classpath separation, testing the installed package) [V]. Flag a mirror tree only when the language does not need it and it has visibly drifted.

### Fixtures and test data

- Fixtures used by one test file go beside it, or in a `__fixtures__/` or `testdata/` folder beside it. Go's toolchain ignores directories named `testdata` [U -- documented in `go help packages`, not fetched this pass].
- Fixtures shared across a package go in one test-support location per package (`tests/common/` in Rust, `conftest.py` in pytest, a `testing/` module in Bulletproof React, a `FeatureTesting` target in Tuist).
- **A finding**: production code that imports from a test-support module. The test helper then ships, and in a bundler it may drag a test framework into the production bundle [I]. Enforce with a boundary rule.
- Large binary fixtures belong in Git LFS or a fetch step, not in the main history (see `build-systems` for fetch-step hermeticity).

### Mocks, fakes, and stubs

- Jest manual mocks [V, jestjs.io/docs/manual-mocks]: `__mocks__/` "immediately adjacent to the module." For node modules, `__mocks__/` goes beside `node_modules` (or under a configured root) and **applies automatically, without a `jest.mock()` call**. "The `__mocks__` folder is case-sensitive." Symptom of misplacement: a node-module mock in a nested `__mocks__/` is ignored, or a root-level one silently replaces a real dependency in every test.
- Tuist `FeatureTesting` and Android test modules give fakes their own build target, so a module's fakes are reusable by other modules' tests without depending on the implementation [V].

### Stories

Colocated `*.stories.tsx` is the Storybook default [V]. A separate `stories/` tree is legitimate when stories compose several features (page-level stories). The failure is a story outside the configured glob, which is invisible, not broken.

### Generated code

- **Mark it.** Go's `// Code generated ... DO NOT EDIT.` line [V]. GitHub's `linguist-generated` attribute in `.gitattributes` hides the file in diffs by default and excludes it from language statistics [V]. Without a marker, reviewers review generated churn, and someone eventually hand-edits a generated file that the next regeneration overwrites.
- **Placement**: colocated per source (GraphQL Codegen's near-operation-file preset writes one file beside each operation [V]) or centralized (`gen/`, `generated/`, `__generated__/`). Colocated output keeps the import short and the relationship obvious. Centralized output makes it easy to ignore, regenerate, and exclude from lint as one glob.
- **Committed or not** is a live disagreement with no primary source fetched [gap]. For committing: Go consumers cannot run your generator, reviewers see the API diff, and a fresh clone builds without the generator installed. Against: merge conflicts in generated files, staleness when someone forgets to regenerate, and build systems (Bazel, Buf) that regenerate anyway. **Committed generated code needs a CI check that regenerates and diffs** [I]. Without one, the checked-in copy and the generator drift. That missing check is the finding, not the choice to commit.
- TypeScript project references need the referenced projects' `.d.ts` output to exist. The docs say to "check in build outputs or build ... after cloning" [V].

### Build output

Maven `target/` [V]. `dist/`, `build/`, `out/`, `.next/`, `target/` in Cargo. All of them must be ignored and must never be imported by source [I]. A source file that imports from `dist/` works until a clean build, and then fails in a way that looks like a missing dependency.

### Docs, ADRs, config, and scripts

- **ARCHITECTURE.md** at the root (matklad) [V].
- **ADRs**: commonly `docs/adr/` or `doc/architecture/decisions/` [U -- adr.github.io and Nygard's 2011 post not fetched]. Consistency matters more than the path.
- **Root config sprawl**: a JavaScript repo root routinely holds 20 or more dotfiles and config files. Some tools let config move into `package.json` or a `config/` folder. Many do not, because they discover config by walking up from the working directory [I]. Do not flag root config count as a finding unless a file is dead.
- **Scripts**: `scripts/` or `tools/`, or `cargo xtask` in Rust (matklad) [V]. The finding is a script that only works from one working directory, not where it lives.

---

## Scale: small versus large

### What the sources say about thresholds

No source gives a reliable line-count or file-count threshold for changing layouts. The numbers that exist:

| Source | Number | What it measures |
|---|---|---|
| Parnas 1972 [V] | 5,000-10,000 instructions | Size where flowchart decomposition was adequate |
| React legacy FAQ [V] | 3-4 nested folders | Maximum nesting depth, from relative-import pain |
| matklad [V] | 10k-1M lines | Range where a flat `crates/` list works |
| matklad [V] | 10k-200k lines | Range where an ARCHITECTURE.md earns its keep |
| Matt Klein [V] | over 100 full-time developers | His meaning of "at scale" for monorepos |
| Android guide [V] | none | "size of the codebase" is the deciding factor |

Treat any other threshold as folklore unless it has a source. "Split when a folder has more than N files" has no evidence behind it [I].

### Small projects

- **Flat is correct until it hurts.** The React FAQ ("start by keeping all files in a single folder"), Abramov ("move files around until it feels right"), and rsc's minimal Go layout all agree [V].
- **Premature structure is a real defect**, not just taste. An eight-folder FSD skeleton or a `domain/application/infrastructure/presentation` hexagonal tree on a small app forces every new file through a classification decision that the team cannot yet make well. FSD v2.1's own authors say finding entities "is still an advanced skill" [V].
- The signal that a small project has outgrown flat: the same few files change together repeatedly and sit far apart, or a reader cannot find the entry point. Not file count.

### Large projects

- **Enforcement becomes mandatory.** Below some size, code review holds boundaries. Above it, nothing but a tool does. Brown: discipline fails "when deadlines loom" [V]. Google: "too easy to add dependencies and reduces the incentive ... to produce stable and well-thought-out APIs," so in 2011 Google set "the default visibility of new APIs to 'private'," and advises such controls "should be put in place as soon as possible" [V].
- **Ownership becomes structural.** Directory ownership files (Google OWNERS, GitHub CODEOWNERS) make the tree an org chart [V for Google]. That is a deliberate Conway alignment.
- **Over-modularization is the large-project failure** that small projects cannot have. Android: too fine-grained means "increased build complexity and boilerplate" [V]. Tuist's interface modules need runtime dependency injection to wire up [V].

### Monorepo versus polyrepo

See Schools of thought. Key data:

- **Google** (Potvin and Levenberg, "Why Google Stores Billions of Lines of Code in a Single Repository," CACM 59(7), July 2016) [V]: about 1 billion files, 35 million commits, 86 TB, about 2 billion lines in 9 million source files (January 2015). Claimed benefits: unified versioning, code sharing, simplified dependency management (no diamond dependencies), atomic changes, large-scale refactoring, flexible team boundaries, and "Code visibility and clear tree structure providing implicit team namespacing." Stated limits: it is "not for everyone," and "would not work well for organizations where large parts of the codebase are private or hidden between groups."

### Modular monolith

- **Shopify packwerk** is **not deprecated**: v3.3.1 shipped 2026-08-26 and the repository is active [V] **VOLATILE**.
- The packwerk retrospective (Salzberg and McGibbon, Shopify Engineering, 2024-02-07) [V] is the best empirical source on enforcing boundaries inside one deployable:
  - Privacy checks were removed in v3.0 because they turned packwerk into "an API design tool." They continue in packwerk-extensions [U].
  - The `app/public` folder convention "broke Rails conventions ... a folder under app that denoted privacy level instead of architecture concepts." **A folder name that encodes visibility instead of meaning is a smell.**
  - Violations reflected bad package graphs: code grouped by semantic name instead of by runtime dependency.
  - A package with zero violations "may actually crash with name errors." Static checks on a dynamic language undercount real dependencies.
  - Packages were used as folders, not boundaries.
  - "Code exerts a powerful drive in the direction of function ... much harder to bend this behavior to fit your mental models than it is to bend your mental models to fit what a codebase actually does."
- That last quote is the empirical case for deriving boundaries from the actual dependency graph (and the change-spread test above) instead of from a whiteboard taxonomy.

---

## Subdomain organization and shared code

### Mapping bounded contexts to folders

- **Phoenix contexts** are the cleanest mainstream mapping: one folder and one public module per context [V].
- **Android data and feature modules** and **Grzybek's modules** are business vertical slices with their own data access [V].
- **Brown's package-by-component** puts each component behind one interface, enforced by visibility [V].
- When contexts are still unclear, a premature context split is expensive to undo, because every cross-context call becomes an API. Draw them with `oo-domain-modeling` first.

### The entity-versus-feature tension

Some code is about a noun used everywhere (`User`, `Order`). Some code is about a verb or a flow (`checkout`, `inviteMember`). Layouts handle the noun differently:

- **FSD**: an `entities/` layer below `features/`, with `@x` for entity-to-entity references [V]. And v2.1 retreated from entity-first decomposition for code that is not reused [V].
- **Redux**: entity slices are just slices. The guide is silent on the distinction [V].
- **Phoenix**: the entity lives inside one context, and other contexts reference it by ID [V].
- **Android**: data modules own entities and expose repositories [V].

A finding here names the symptom: a `user/` folder that every feature imports and that every feature edits. That is an entity with no owner. Give it an owner, or split the parts each feature actually uses.

### Cross-feature dependencies: how each convention answers

| Convention | Rule |
|---|---|
| Bulletproof React | Banned. Compose at app level [V] |
| FSD | Only downward across layers. Same-layer slices cannot import each other. `@x` for entities [V] |
| Android | Feature modules do not depend on each other. App mediates via navigation with IDs [V] |
| Tuist TMA | Depend only on `Interface` targets [V] |
| Redux style guide | `/app` depends on all folders. Cross-slice reaction via `extraReducers` [V] |
| Phoenix | Contexts call each other's public module, never their schemas [V] |
| Google monorepo | Visibility private by default, opened per consumer [V] |

Every convention bans or narrows feature-to-feature imports. None of them allows them freely. **A feature-folder layout with no cross-feature rule is the most common way a by-feature migration fails to deliver** [I].

### The shared/common/utils dumping ground

- Go: "Packages named util, common, or misc provide clients with no sense of what the package contains" [V].
- Redux: `/common` is for code that is "truly generic and reusable" [V].
- FSD: `shared` may hold infrastructure (route constants, API clients, the logo) but no business logic [V].
- Dodds: centralized utilities outlive their callers [V].
- Google: shared code is "too easy to add" as a dependency [V].

Checks for a shared folder [I]:
1. **Single-consumer exports.** An export imported by one feature belongs in that feature.
2. **Domain words.** A shared file that mentions a domain noun (`formatInvoiceTotal`) is not generic.
3. **Inbound edges from shared into features.** Shared must not import a feature. That edge creates a cycle and means the "shared" code is not shared.
4. **Size and churn.** The shared folder with the highest commit rate in the repo is a missing module, not a utility belt.

The fix is rarely "delete `utils/`." It is "give each cluster in `utils/` a name that says what it hides" (`stringset`, `money`, `retry`), which is the Go blog's worked example.

### Folders per layer inside every feature

A feature folder that contains `controllers/`, `services/`, `repositories/`, `dto/`, `mappers/` for a feature with three files is ceremony. Bogard argues against forcing shared layers into every slice [V]. A primary source for the explicit criticism was not found [gap]. Flag it only with the cost visible: the feature has more folders than files, or most folders hold one file.

---

## Enforcing boundaries

The point of this section: **a layout recommendation without an enforcement mechanism is incomplete.** For each ecosystem, know the cheapest tool.

| Ecosystem | Built-in | Tooling |
|---|---|---|
| Go | `internal/` (compiler) [V] | -- |
| Rust | crate boundary, `pub(crate)`, `pub(super)` [V] | -- |
| Java | package-private, JPMS `exports` [V via Brown] | ArchUnit [V] |
| .NET | `internal` per assembly [V] | -- |
| Kotlin / Android | Gradle modules, `implementation` vs `api` [V] | -- |
| TypeScript | project references with `composite` and `tsc -b` "Enforce logical separation" [V] | `import/no-restricted-paths` (Bulletproof) [V], eslint-plugin-boundaries [V], dependency-cruiser [V], Nx `@nx/enforce-module-boundaries` [V], Steiger for FSD [V] |
| Python | -- | import-linter [V] |
| Ruby | -- | packwerk [V], pks (a Rust reimplementation) [V] |
| Swift | modules / targets, `internal` default access | Tuist [V] |

Nx detail [V]: boundaries use project tags and `depConstraints` with `onlyDependOnLibsWithTags`. "Projects without any tags cannot depend on any other projects" once constraints are on. That default is what makes it a boundary instead of a suggestion.

**The enforcement checks to run in review** [I]:
1. Is there a rule at all?
2. Is it in CI, or only in an editor plugin?
3. Does it have an allow-list of existing violations, and is the list shrinking or growing?
4. Does it cover type-only imports? Some rules skip `import type`, which lets a type dependency cross the boundary and later become a value dependency.
5. Does it see dynamic imports, `require`, and path aliases? A rule that matches relative paths only is defeated by an alias.

---

## Barrel files and index re-exports

A barrel is an `index.ts` (or `__init__.py`, or `mod.rs` with `pub use`) that re-exports a folder's contents.

### The evidence against

- **Marvin Hagemeister**, "Speeding up the JavaScript ecosystem, part 7: The barrel file debacle" (2023-10-08) [V]. Module count against load time:

  | Modules | Load time |
  |---|---|
  | 500 | 0.15 s |
  | 1,000 | 0.31 s |
  | 10,000 | 3.12 s |
  | 25,000 | 16.81 s |
  | 50,000 | 48.44 s |

  Load time grows faster than linearly. With 100 test files at 4-way parallelism, the overhead reaches about 1 min 18 s, 7 min, and 20 min at the larger sizes. Test runners that isolate each test file (Jest, Vitest) pay the load per file. Hagemeister's conclusion: a "free optimization ... 60-80% faster: Get rid of all barrel files."
- **Atlassian**, "Faster builds when removing barrel files" (Tim Sebastian, 2025-06-26) [V]. The Jira frontend, "thousands of internal packages," about 100,000 files changed by a codemod (a fixable ESLint rule).
  - "75% faster builds." Build runtime down 73%.
  - TypeScript highlighting more than 30% faster. Local unit tests about 50% faster.
  - Tests selected per change fell from 1,600 to 200, and integration tests triggered fell 85%, because precise import edges gave precise test selection.
  - **Atlassian's own stated downsides**: "Packages can no longer easily control their 'public API' through barrel files," and "moving source files now requires updating every direct import, making refactoring more fragile."
- **Bulletproof React** reversed its barrel recommendation over Vite tree-shaking [V].
- **Next.js `optimizePackageImports`** rewrites barrel imports for a default list of packages (lucide-react, date-fns, lodash-es, @mui/*, rxjs, effect, and others). It is still "experimental ... not recommended for production" [V] **VOLATILE**. That a framework ships a compiler pass to undo barrels is evidence of the cost.
- **eslint-plugin-barrel-files**: rules `avoid-barrel-files`, `avoid-importing-barrel-files`, `avoid-namespace-import`, `avoid-re-export-all` [V]. The repository has been quiet since 2025-02 **VOLATILE**.

### The case for

- **A barrel is the public API of a module.** FSD requires one per slice: "a contract between a group of modules ... and the code that uses it," protecting consumers from internal moves and keeping exposure minimal [V].
- **Atlassian's downsides list is the pro-barrel argument**, stated by the team that removed them [V]: without a barrel, there is no single place that defines what a package exposes, and every internal move breaks consumers.
- In a language with no visibility enforcement (TypeScript), the barrel plus a lint rule ("import only from the index") is the cheapest way to make a folder a boundary [I].

### Hazards either way

- **Circular imports**: a file inside a slice that imports its own slice's barrel gets a partially initialized module, and the symptom is a value that is `undefined` at import time and defined later [V, FSD]. FSD's rule: never import your own slice through its index.
- **`export *`** hides what a module exposes, defeats "find all references" in some editors, and silently adds new exports to the public surface when an internal file adds one [I].
- **Middle paths**: `package.json` `exports` maps for internal packages (explicit public surface, no runtime re-export file) [I], or one index per component rather than one per folder (FSD's mitigation for `shared/ui`) [V].

The review question is not "barrels yes or no." It is: **how big is the module graph, does the test runner isolate per file, and what else enforces the public API if the barrel goes?**

---

## Moves, renames, and history

Restructuring is a refactor of the tree. It has the same discipline as any refactor: separate the move from the behavior change.

- **Rename detection is similarity-based** [I]. Git records no rename. It infers one when a deleted path and an added path are similar enough. A commit that moves a file *and* rewrites half of it can fall below the threshold, and history appears to start at the new path.
- **Make move-only commits.** Then `git log --follow` (one file at a time) and `git blame` follow the move [I on `--follow` limits].
- **`git blame -C`** detects lines moved or copied from other files in the same commit. `-C -C -C` searches all commits [V].
- **`blame.ignoreRevsFile`** names commits for blame to skip. GitHub honors a root `.git-blame-ignore-revs` file [V]. Add mass-move and mass-format commits to it.
- **Mass moves and open branches**: every open branch that touches a moved file gets a conflict. Land the move when few branches are open, and announce it [I].
- **Path aliases diverge between tools.** TypeScript `paths` "does not change how import paths are emitted ... another tool has this mapping" [V]. The bundler, Jest `moduleNameMapper`, Vitest, ESLint's import resolver, and Node each need their own copy [I]. Symptom: the editor resolves an import, the test runner does not. TypeScript 6.0 (2026-03-23) deprecates `baseUrl` (removed in 7.0) and `moduleResolution: node10` [V] **VOLATILE**. Node's `package.json` `imports` field (`#internal/*`) is one mapping that Node and TypeScript both read [U].
- **Case-only renames** on case-insensitive filesystems: route to `naming-conventions`.

---

## Schools of thought (preserve disagreement)

These are unresolved. State each side at full strength. Do not average them.

### By feature versus by layer versus by type

- **By feature.** Redux style guide (B-level), Angular's current guide, Bulletproof React, Hauer, Martin's "Screaming Architecture," Bogard. Argument: change locality (CCP), discoverability, deletion of whole features, team ownership. The domain is visible at the top.
- **By layer / by type.** The historical default of Rails, Maven-era Java, and most tutorials. Fowler, per Brown, says layering is a fine place to start [U on the exact wording]. Argument: one dependency direction is visible and enforceable across the whole app, every developer knows where a kind of thing lives in any project ("site-wide look-and-feel," Maven), and by-type has zero classification cost because the kind of a file is never in doubt, while its feature often is.
- **Brown's dissent from both**: "both are suboptimal." The axis matters less than whether visibility makes the boundary real.
- **When each is right**: by-type in small apps and in framework-convention apps where the convention is the shared language. By-feature once feature count makes cross-folder change spread the dominant cost. Brown's position whenever the language can enforce visibility.

### Colocation versus separation

- **Colocate**: Dodds, Storybook, Angular, Go, Rust unit tests, Next.js private folders, FSD v2.1. Argument: what changes together lives together, deletion is complete, and nothing is orphaned.
- **Separate**: Maven, pytest with src layout, Rust `tests/`. Argument: separation is not taste. It changes what the test can see. A separate tree tests the public API or the installed artifact, which a colocated test cannot. It also keeps test-only dependencies out of the production classpath or bundle without per-tool exclusion globs.

### Flat versus nested

- **Flat**: matklad ("even comparatively large lists are easier to understand at a glance than even small trees"), the React FAQ's 3-4 level cap, rsc's minimal Go layout. Argument: a list is scannable, a tree hides things, and every nesting level is a classification decision someone can get wrong.
- **Nested**: FSD's layers, slices, and segments. Bulletproof's feature internals. Argument: a fixed hierarchy tells a newcomer where any new file goes, and a flat list of 200 entries has no grouping signal.
- Parnas sits outside both: hierarchy and clean decomposition are "independent properties" [V].

### Monorepo versus polyrepo

- **Matt Klein, "Monorepos: Please don't!"** (2019-01-02) [V]: "a monorepo must solve every problem that a polyrepo must solve, with the downside of encouraging tight coupling, and the additional herculean effort of tackling VCS scalability." Atomic refactors are a "fallacy" because deploys are staggered. "Polyrepo code layout offers clear team/project/abstraction/ownership boundaries and encourages developers to think carefully about contracts." At Twitter, `git status` took minutes. The outcome is "a direct result of engineering culture and leadership."
- **Adam Jacob, "Monorepo: please do!"** (2019-01-03) [V]: the default behavior a monorepo encourages "is visibility and shared responsibility," and "technically ... a wash." "It forces the conversation, and makes trade-offs visible." A polyrepo fork hides duplication in a long-lived branch. In a monorepo "this pain is direct and up front. It sucks more, and that's a good thing."
- **Google** (2016) [V]: strong benefits at its scale, with custom tooling, and "not for everyone."
- Note what both Klein and Jacob concede: the tooling and culture decide the outcome more than the repo count.

### Prescriptive methodology versus emergent structure

- **Prescriptive**: FSD, Bulletproof React, Tuist TMA, Android's module types. Argument: a shared, written rule ends layout debates, makes code review mechanical, and lets a linter enforce it.
- **Emergent**: Abramov, the React FAQ, Phoenix's admission that contexts are "hard to draw," the packwerk retrospective ("bend your mental models to fit what a codebase actually does"). Argument: the right boundaries are discovered from the change history and the dependency graph, and a taxonomy imposed early encodes guesses.
- **FSD v2.1 moved toward the emergent camp** on single-use code [V]. That is evidence about the cost of early classification, from the prescriptive side.

### Convention-first versus domain-first top level

- **Convention-first** (Rails, Maven, Next.js `app/`, Nuxt): the framework owns the top level, and every project in the ecosystem looks alike. Rails: "You're not a beautiful and unique snowflake" [V].
- **Domain-first** (Screaming Architecture): the top level shows the business, and the framework is a detail [V].
- In frameworks whose directories carry behavior (Next routing, Nuxt auto-import), domain-first is only possible below the framework-owned level.

### Barrel files

See the barrel section. Hagemeister, Atlassian, and Bulletproof against. FSD and the "public API" argument for. Atlassian's own downsides list is the strongest pro-barrel text, written by the side that removed them.

### Generated code committed or not

See Generated code. No primary source pins either side. The defensible position on either side includes a regenerate-and-diff check.

---

## Anti-pattern catalog

Each entry: the pattern, the trigger, the consequence, the fix.

### Boundaries

- **Folder-only boundary.** Trigger: a `features/` or `packages/` tree with no import rule. Consequence: features import each other's internals, and the tree looks modular while the graph is a tangle (packwerk's "packages as folders"). Fix: add the cheapest enforcement for the ecosystem (table above) in CI, with a shrinking allow-list.
- **Sideways feature imports.** Trigger: `features/a` imports `features/b/components/Thing`. Consequence: a change to b's internals breaks a. The features cannot be deleted or owned independently. Fix: compose in `app/` or a page, move the shared part down a layer, or expose it through b's public API.
- **Shared imports a feature.** Trigger: `common/` or `shared/` imports from `features/`. Consequence: a cycle, and the "shared" code carries one feature's domain. Fix: move the code into the feature, or invert the dependency.
- **Visibility encoded as a folder name.** Trigger: `app/public/`, `internal/` used as a convention without a compiler that enforces it, `_private` without a lint rule. Consequence: the name promises a boundary that nothing checks. Packwerk's retrospective named this exact pattern [V]. Fix: enforce it or drop the name.
- **Cross-feature rule that ignores aliases or `import type`.** Consequence: the rule passes while the boundary leaks. Fix: test the rule with a deliberate violation through an alias.

### Shared code

- **Utility dumping ground.** Trigger: `utils/`, `helpers/`, `common/misc`. Consequence: no cohesion, dead code accumulates, every feature depends on it so it can never change. Fix: name each cluster for what it hides. Move single-consumer exports into their consumer.
- **Domain logic in shared.** Trigger: `shared/formatInvoice.ts`. Consequence: the billing concept leaks into every feature's dependency set. Fix: move it into the billing feature or entity.
- **Ownerless entity folder.** Trigger: a `user/` module that every feature edits. Consequence: merge conflicts and no one accountable for its model. Fix: assign an owner. Split per-feature projections.

### Axis and scale

- **By-type at scale.** Trigger: `components/`, `hooks/`, `reducers/`, `api/` at the top with dozens of features. Consequence: high change spread, no feature can be deleted cleanly. Fix: migrate by feature, one feature per change, starting with the most active.
- **Duplicated tree per type.** Trigger: `components/todos/` + `reducers/todos/` + `api/todos/`. Consequence: the feature exists as a folder name repeated in four places, with the cost of both axes. Fix: invert to `features/todos/{...}`.
- **Premature methodology.** Trigger: full FSD or hexagonal skeleton on a small or new app. Consequence: classification debates on every file, single-file folders, and code placed by guess. Fix: start flat, extract on evidence. Cite FSD v2.1 if the team cites FSD.
- **Layer folders inside every feature.** Trigger: more folders than files in a feature. Consequence: ceremony that hides the three files that matter. Fix: collapse until a folder earns its keep.
- **Over-modularization.** Trigger: one build module per screen, with interface modules nobody substitutes. Consequence: build config and dependency-injection wiring cost more than they save (Android "too fine-grained") [V]. Fix: merge modules that always change together.

### Artifacts

- **Flat Python layout testing the working tree.** Consequence: tests pass while the built wheel is missing files. Fix: src layout, or run tests against an installed build in CI.
- **`tests/common.rs` in Rust.** Consequence: an extra empty test crate, and the helpers are not shared as intended. Fix: `tests/common/mod.rs`.
- **Test-support code imported by production code.** Consequence: test helpers ship, sometimes with the test framework. Fix: a boundary rule that forbids the edge.
- **Misplaced Jest node-module mock.** Consequence: the mock is ignored, or applies to every test without a `jest.mock()` call. Fix: put it beside `node_modules` or the configured root, on purpose.
- **Unmarked generated code.** Consequence: reviewers review generated churn, and someone hand-edits output that the next generation overwrites. Fix: the language marker plus `linguist-generated`.
- **Committed generated code with no drift check.** Consequence: the checked-in copy and the generator disagree, and nobody knows which one is true. Fix: CI regenerates and fails on diff.
- **Source importing build output.** Trigger: `import ... from '../dist/...'`. Consequence: works until a clean build. Fix: import the source or the package name.
- **Story or test outside the discovery glob.** Consequence: invisible, not failing. Fix: check globs when adding a new top-level folder.

### Moves

- **Move and edit in one commit.** Consequence: history appears to start at the new path. Fix: separate move-only commits, and list mass moves in `.git-blame-ignore-revs`.
- **Alias added to one tool only.** Consequence: the editor resolves, the tests or the bundle do not. Fix: one source of truth (`package.json` `imports`, or a shared config generated for each tool).
- **Moving a file out of a framework-owned directory.** Trigger: Nuxt `composables/`, Next.js `app/` route segments. Consequence: behavior changes (auto-import lost, a route appears or disappears). Fix: treat the move as a behavior change and test it.

### Barrels

- **Barrel at every folder in a large graph.** Consequence: test and build time grows with total module count, not the module actually used (Hagemeister, Atlassian). Fix: direct imports, or one barrel per public package boundary only.
- **Self-import through own barrel.** Consequence: circular import, a value is `undefined` at module evaluation. Fix: import siblings by relative path.
- **`export *` from internals.** Consequence: every internal export becomes public API without a decision. Fix: named re-exports only.

### Org and history

- **Tree mirrors a past org chart.** Trigger: top-level folders named for teams that no longer exist. Consequence: ownership files point at nobody, and the tree misleads about responsibility. Fix: re-align with current ownership or with the domain, in move-only commits.

---

## What is NOT a project-structure finding

- A layout that differs from the reviewer's preference but follows the project's own documented convention consistently. Project conventions win (see `panel-contract.md`).
- File count in a folder, absent a change-spread or findability symptom.
- Root config sprawl where every file is live and tool-mandated.
- By-type folders in a small app that is not growing.
- Barrel files in a small module graph where nobody has measured a cost.
- A framework's required directories, even when they break by-feature purity.
- Identifier and file-name casing (route to `naming-conventions`).
- Build-graph correctness of a module split (route to `build-systems`).

---

## Severity calibration (this domain)

- **blocker**: a structure change that silently ships or tests the wrong thing. A Python flat layout where CI tests the working tree and the wheel is the release artifact, with a known missing file. A move out of a framework-owned directory that drops a route or an auto-import in production. Generated code edited by hand where regeneration will overwrite a production fix.
- **major**: a boundary that the change itself breaks or makes unenforceable. A new sideways feature import in a codebase that bans them. A new dependency from `shared/` into a feature (a cycle). A new module with no enforcement in a project where every other module has it. A mass move combined with edits that destroys history for a large subsystem. A barrel added at a hot boundary in a graph where test time is already a known problem.
- **minor**: a placement inconsistent with the project's convention (one feature organized by type in a by-feature app). A new utility in a dumping ground with one consumer. A test-support folder in a non-standard place that the tooling still discovers.
- **nit**: folder order, a single extra nesting level, an empty folder.
- **insight**: an observation about axis fit for the project's current size, with change-spread evidence. A suggestion to adopt an enforcement tool, with the specific rule. A note that the project now needs REP/CRP grouping where it was built for CCP.

Confidence: high when the finding cites an import edge, a tool's documented discovery rule, or a measured change spread. Medium when it argues from the axis model without evidence from this repo's history. Low when it relies on a volatile framework convention that was not checked against the installed version.

---

## Authorities

| Source | Use it for |
|---|---|
| David Parnas, "On the Criteria To Be Used in Decomposing Systems into Modules" (1972) | The root criterion: decompose by hidden decisions, not processing steps |
| Melvin Conway (1968), Martin Fowler's ConwaysLaw bliki | Org-structure mapping and the Inverse Conway Maneuver |
| Robert Martin, *Agile PPP* (2002), *Clean Architecture* (2017) | Package cohesion and coupling principles, stability metrics |
| Simon Brown, *Clean Architecture* ch. 34 | Visibility as the real architecture. The best single source for this lens |
| John Ousterhout, *A Philosophy of Software Design* | Deep modules, temporal decomposition |
| Robert Martin, "Screaming Architecture" (2011) | Domain-first top level |
| Jimmy Bogard, "Vertical Slice Architecture" (2018) | Slices, coupling inside versus between |
| Kamil Grzybek, "Modular Monolith: A Primer" (2019) | Modules inside one deployable |
| Redux Style Guide, RTK and RTK Query docs | Frontend feature-folder canon with explicit priority levels |
| Dan Abramov, React legacy FAQ, Kent C. Dodds "Colocation" | The emergent and colocation school |
| Feature-Sliced Design docs and release notes | The most prescriptive frontend spec, and its own v2.1 self-critique |
| Bulletproof React | A pragmatic template, and a documented barrel reversal |
| Next.js, Nuxt, Angular, Storybook docs | Framework-owned conventions (volatile) |
| Russ Cox, project-layout issue #117; go.dev "Organizing a Go module"; Go blog "Package names" | Debunking cargo-cult Go layouts, `internal/`, package naming |
| Aleksey Kladov (matklad), "Large Rust Workspaces", "ARCHITECTURE.md" | Rust workspace shape, the codemap document |
| The Rust Book ch. 11.3, Rust Reference modules | Test placement, module file conventions |
| PyPA src-layout discussion, pytest Good Integration Practices, import-linter | Python layout and enforcement |
| Gradle multi-project docs, Maven Standard Directory Layout | JVM layout and convention plugins |
| Android modularization guide, Tuist TMA docs | Mobile module types and interface modules |
| Phoenix contexts guide | Folder-equals-bounded-context |
| Potvin and Levenberg, CACM 2016; Matt Klein; Adam Jacob | Repo strategy, both sides |
| Shopify packwerk retrospective (2024) | Empirical lessons on enforcing boundaries in a monolith |
| Marvin Hagemeister (2023), Atlassian Engineering (2025) | Barrel-file performance data |
| git-blame docs, GitHub blame-ignore docs | History survival across moves |

---

## Process for the project-structure agent

1. **Identify the ecosystem, framework, and versions.** Framework-owned directories (Next.js, Nuxt, Angular, Rails, Maven) constrain the layout and change between majors.
2. **Read the project's own convention** (CLAUDE.md, CONTRIBUTING, ARCHITECTURE.md, an existing lint rule). Consistency with a documented convention outranks a better convention.
3. **Name the axis at each level** of the tree.
4. **Find the enforcement.** What stops a cross-boundary import? If nothing, every other finding is about labels.
5. **Walk the import edges the change adds**: sideways between features, upward from shared, into test support from production.
6. **Walk shared folders** for single-consumer exports, domain words, and inbound edges.
7. **Walk artifact placement** against each tool's discovery rule: tests, fixtures, mocks, stories, generated files, build output.
8. **Check moves** for move-only commits, alias coverage in every tool, and framework-owned directories.
9. **Check barrels** against graph size and runner isolation, and for self-imports.
10. **Size the recommendation to the project.** Measure change spread when proposing an axis change.
11. **Route** casing to `naming-conventions`, build-graph mechanics to `build-systems`, context boundaries to `oo-domain-modeling`, dependency direction between classes to `oo-architecture`.
12. **Stay read-only.**

---

## Changelog

**Source research**: `~/.claude/local/research-notes/project-structure-research.md` (claims tagged VERIFIED / FOUND-UNVERIFIED / INFERRED, with a gaps list). Read it before a refresh.

- **2026-10-02** -- Initial version. Verified at source: Parnas 1972, Conway 1968, Brown ch. 34 (via book text), Redux style guide priorities and example tree, ducks repo, RTK Query rules, React legacy FAQ, Dodds, Bulletproof React (including barrel reversal), FSD layers and v2.1 release notes, Next.js 16.3 project structure, Nuxt 4, Angular v20 suffix removal, Storybook 10.6, rsc issue #117, go.dev layout and package-names post, Rust Reference and Book, matklad posts, PyPA, pytest, import-linter v2.15, Gradle 9.8, Android modularization, Tuist TMA, Phoenix 1.8, Google CACM 2016 (via headless browser), Klein, Jacob, packwerk v3.3.1 and retrospective, Hagemeister, Atlassian, Next.js `optimizePackageImports`, TS 6.0 deprecations, Jest manual mocks, `linguist-generated`, git blame. Known gaps: Martin's principles and Ousterhout from secondary sources only; JPMS and .NET layouts not researched; committed-generated-code debate has no primary source; ADR location conventions, Node `#imports`, Go `testdata` rule, Inverse Conway coinage, and atomic-design criticism unverified.
