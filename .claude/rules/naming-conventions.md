---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-02
---

# Naming Conventions

A reference for reviewing naming at system scope: per-ecosystem conventions, consistency across a codebase, vocabulary discipline, and names that cross a language, protocol, storage, or tool boundary. Used by the `naming-conventions` subagent.

Distinct from:
- **`readability`**: whether one identifier is clear at its point of use (descriptive, pronounceable, right altitude). This file owns whether the name *follows the convention*, *matches the rest of the system*, and *survives the boundaries it crosses*. Clean Code ch. 2 splits cleanly: "Pick One Word per Concept" and "Don't Pun" belong here. The rest belongs to `readability`.
- **`project-structure`**: what a directory or module *means* (a `utils/` package is a cohesion problem there). This file owns how file and directory names are *spelled*, file names that carry build or framework semantics, and case-only renames.
- **`api-design`**: the shape and evolution of a public contract. This file owns the spelling and vocabulary of its fields, and the cost of renaming them. **Shared seam**: a rename of a public field is both a naming finding and a breaking change. Name the seam.
- **`web-analytics`**: event and property taxonomy for analytics. This file defers to it on analytics event names.
- **`otel-instrumentation`**: span and attribute names under OpenTelemetry semantic conventions. This file covers the Prometheus-versus-OTel conflict only as a boundary hazard.
- **`content-design`** and **`information-architecture`**: words a user sees. This file owns words a program or a developer sees.
- **`i18n`** and **`text-engineering`**: Unicode in identifiers is out of scope here except where a tool restricts it.

Verification markers: **[V]** verified against a primary source on the `last-verified` date, **[V-exp]** verified by a local experiment (macOS APFS, git 2.50.0, Pydantic 2.13.5), **[U]** found but not verified against a primary source, **[I]** inferred by the author.

---

## Thesis

**A name is a contract that crosses more boundaries than the code that defines it, and convention is what makes a name guessable.**

Two empirical facts carry the lens.

First, **developers rarely choose the same name.** Feitelson et al., "How Developers Choose Names" (IEEE TSE, arXiv 2103.07487) [V]: across 334 subjects, the median probability that two developers pick the same name for the same thing is **6.9%**. Furnas et al. (CACM 1987) found under 0.2 agreement on command words [V via Feitelson]. Synonym drift is the default outcome, not a lapse. A convention and a glossary are the only things that move the number. Simonyi set the same target in the 1970s: imagine a reward when two programmers independently write the same text [V].

Second, **a name outlives its type.** Inside one language the compiler checks the name. At a boundary (JSON key, database column, environment variable, CLI flag, HTTP header, metric, story ID, URL path) nothing checks it, conversion between casing styles is lossy, and many decoders silently drop a key they do not recognize. Hyrum's Law makes every such name a contract: "all observable behaviors ... will be depended on by somebody" [V].

**The operational question**: can someone who has never seen this name guess it from the rest of the system, and does it keep one spelling and one meaning across every boundary it crosses?

### Empirical priority order

These bite most often, and hardest, in this order. Triage in this order.

1. **Names that change at a boundary.** A camelCase key read by a snake_case decoder, a column name that Postgres folded to lowercase, a header looked up by exact case behind an HTTP/2 proxy. The symptom is silent: a field is `undefined`, `None`, or zero, and no error is raised.
2. **One concept, several words (synonyms), or one word, several concepts (homonyms).** `user` / `account` / `member` for one entity. `fetch` / `retrieve` / `get` for one action. `meter` meaning two things in two contexts. Symptom: duplicated logic, and two teams who think they disagree when they do not.
3. **Names that lie about behavior** (Arnaoudova's linguistic antipatterns). `getX` that does IO, `isX` that returns a non-boolean, `containsX` that returns the found item. Readers trust the verb and skip the body.
4. **Names that a tool or framework reads.** Go `_test.go` and `_windows.go` suffixes, files starting with `_`, Rails' `type` column, Zeitwerk acronym inflections, JavaBeans getter introspection, Storybook export names as story IDs, test-runner prefixes. A rename changes behavior.
5. **Units and kinds missing at untyped boundaries.** `timeout: 30` in a JSON config, a `duration` column, a `RETRY_DELAY` environment variable. Mars Climate Orbiter is the archetype.
6. **Acronym casing split.** `userId` and `userID` in one codebase, so grep finds half the uses. Three incompatible camps exist across ecosystems.
7. **Casing style that breaks the ecosystem's canon.** `get_user` in a Java codebase, `GetUser` in Python. Real but cheap: linters catch most of it.
8. **Case-only renames on case-insensitive filesystems.** A plain `mv` that git does not see, then a Linux CI failure.

---

## Volatile surface

`last-verified` (see frontmatter -- do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| typescript-eslint `naming-convention` options (rule is feature-frozen) | Medium | typescript-eslint.io/rules/naming-convention |
| Biome `useNamingConvention` defaults (`strictCase`, `requireAscii`) | Medium | biomejs.dev/linter/rules/use-naming-convention |
| eslint-plugin-unicorn `filename-case` cases and config membership | Medium | github.com/sindresorhus/eslint-plugin-unicorn docs/rules/filename-case.md |
| Clippy lint group membership (lints move between groups) | Medium | rust-lang.github.io/rust-clippy/master |
| Ruff `N` rules | Slow | docs.astral.sh/ruff/rules/#pep8-naming-n |
| SwiftLint `identifier_name` defaults | Slow | realm.github.io/SwiftLint/identifier_name.html |
| Checkstyle naming checks | Slow | checkstyle.org/checks/naming |
| TypeScript `forceConsistentCasingInFileNames` default | Medium | typescriptlang.org/tsconfig |
| Pydantic alias defaults (`validate_by_alias` / `serialize_by_alias`, V3 change announced) | Fast | pydantic.dev/docs/validation/latest/concepts/alias |
| Go `encoding/json` v2 case sensitivity and release status | Medium | pkg.go.dev/encoding/json |
| .NET Framework Design Guidelines (2nd ed online, 3rd ed print only) and TAP page | Slow | learn.microsoft.com/dotnet/standard/design-guidelines |
| Kubernetes name validation (relaxed-validation feature gate) | Medium | kubernetes.io/docs/concepts/overview/working-with-objects/names |
| Prometheus and OTel metric naming, and the translation between them | Medium | prometheus.io/docs/practices/naming, opentelemetry.io/docs/specs/semconv/general/naming |
| Elixir and Pydantic doc URLs (both moved recently) | Fast | as above |
| git case handling, reftable backend | Slow | `git help config` |

PEP 8, the Rust API Guidelines, Effective Go, the Swift API Design Guidelines, the RFCs, POSIX, ISO C, and the empirical papers are stable.

---

## Lineage: why the conventions look the way they do

### Hungarian: Apps versus Systems

Charles Simonyi, "Hungarian Notation" (MSDN reprint, 1999, from his thesis) [V]. Simonyi defines a type as "the set of operations that can be applied to a quantity," which is wider than representation: two integers `x` and `y` are different types if `Position(x, y)` is legal and `Position(y, x)` is nonsense. The goal is checks "very similar to the 'dimension' checks in physics."

Joel Spolsky, "Making Wrong Code Look Wrong" (2005-05-11) [V], named the split. **Apps Hungarian** (from the Excel and Word applications division) prefixes the *kind*: `rw` / `col` for row and column, `xl` / `xw` for layout and window coordinates, `us` / `s` for unsafe and safe strings. **Systems Hungarian** prefixes the *representation* (`dw`, `ul`, `sz`), which Spolsky calls "a subtle but complete misunderstanding." Charles Petzold's *Programming Windows* spread the Systems dialect [U].

What survives today [I]:
- Systems Hungarian is dead everywhere. The Linux kernel calls it "asinine" [V]. .NET bans it [V]. Google TypeScript bans it [V].
- **Apps Hungarian's intent is correct and now lives in types**: newtypes in Rust, branded types in TypeScript, `Duration` instead of `int`. Where no type can carry the kind (JSON, SQL, env vars, flags, metrics), the kind or unit goes in the name as a suffix. That is Apps Hungarian, renamed.

Simonyi's interval qualifiers [V] are still the clearest vocabulary for ranges:
- `XFirst`: first element.
- `XLast`: last element, closed (`x <= xLast`).
- `XLim`: strict, half-open limit (`x < xLim`).
- `XMax`: allocated limit.
- `XMac`: current count.

The `last` versus `end` / `lim` distinction is the naming root of a family of off-by-one bugs [I]. Simonyi also says a flag "should describe the true state of the flag" [V], which is positive boolean naming from the 1970s.

### Short names: Pike, C, and scope

- Rob Pike, "Notes on Programming in C" (1989) [V]: "Length is not a virtue in a name; clarity of expression is." And the condition people omit: "np is just as mnemonic as nodepointer **if you consistently use a naming convention**." **Short names depend on a convention.** Pike also writes that embedded capitals "jangle like bad typography." Go later required MixedCaps because export is encoded in the case of the first letter.
- Kernighan and Pike, *The Practice of Programming* (1999): "Use descriptive names for globals, short names for locals" and "Do the same thing the same way everywhere" [V via the appendix rule list].
- Pre-C99 C guaranteed only 6 significant characters for external identifiers (31 internal). C99 raised this to 31 and 63 [V, cppreference]. That is why `strcpy` and `creat` look the way they do. It is a mechanical constraint, not a style.

### Where the casings came from

- **camelCase** started with Mesa at Xerox PARC around 1978. The Alto keyboard had `←` where the underscore sits, and hyphen and space were illegal in identifiers [V, secondary]. Smalltalk used it. Niklaus Wirth took it to Modula. "InterCaps" dates from 1990, "CamelCase" from 1995, and "PascalCase" from .NET design discussions [V, secondary].
- **snake_case**: underscores in identifiers go back to the late 1960s and C (1978). The name "snake_case" is from Gavin Kistner on Ruby Usenet, 2004 [V, secondary].
- **kebab-case** exists only where the grammar has no infix minus: Lisp, COBOL, CSS, HTML attributes, URLs, CLI flags, YAML keys, package names [I].
- **Smalltalk keyword selectors** (`at:put:`) became Objective-C `setObject:forKey:`, and then Swift argument labels [I].

---

## Per-ecosystem canon

These are the rules a reviewer must know exactly, because "follow the ecosystem" is the default rule. One company can hold different rules per language: Google's C++ guide uses PascalCase functions and `kName` constants, while its Java guide uses camelCase and `UPPER_SNAKE` [V]. **Do not port a convention across languages.**

### Rust

Rust API Guidelines [V, rust-lang.github.io/api-guidelines/naming.html]:
- **C-CASE** (RFC 430): types, traits, enum variants `UpperCamelCase`. Modules, functions, methods, macros `snake_case`. Consts and statics `SCREAMING_SNAKE_CASE`. Type parameters `T`, lifetimes `'a`.
- Acronyms "count as one word: use `Uuid` rather than `UUID`."
- Constructors `new` or `with_*`. Conversion constructors `from_*`.
- **C-CONV**: `as_` is free and borrowed-to-borrowed. `to_` is expensive. `into_` consumes `self`. `into_inner()` unwraps a wrapper.
- **C-GETTER**: no `get_` prefix, except for a "single and obvious thing" (`Cell::get`). Unsafe unchecked variants are `get_unchecked`.
- **C-ITER**: `iter`, `iter_mut`, `into_iter`, and iterator types named to match (`IntoIter`).
- **C-FEATURE**: name a feature `std`, not `use-std`. Features are additive, so `no-abc` does not work.
- **C-WORD-ORDER**: verb-object-error (`ParseIntError`).
- No `-rs` suffix on crate names.
- **Units moved from the name to the type**: `thread::sleep_ms(ms: u32)` is "Deprecated since 1.6.0: replaced by `std::thread::sleep`," which takes a `Duration` [V].

Clippy's `wrong_self_convention` [V, source] enforces C-CONV: `as_` takes `&self` or `&mut self`, `from_` takes no `self`, `into_` takes `self`, `is_` takes `&self` or none, `to_*_mut` takes `&mut self`, and `to_` takes `self` on `Copy` types and `&self` otherwise.

### Go

Effective Go, Go Code Review Comments, Andrew Gerrand's "What's in a name?" (2014), and the Google Go Style Guide [V]:
- Package names: lowercase, single word, no underscores or mixedCaps, equal to the directory's base name.
- **Avoid stutter**: `bufio.Reader`, not `bufio.BufReader`. `ring.New`, not `ring.NewRing`. The package name is part of every reference.
- Banned package names: `util`, `common`, `misc`, `api`, `types`, `interfaces` (the meaning side belongs to `project-structure`).
- Getters: `Owner()`, not `GetOwner()`. "Prefer `Compute` or `Fetch`" for expensive operations, so the reader knows the call is not a cheap field read.
- One-method interfaces take `-er` (`Reader`). **Canonical names carry canonical signatures**: "call your string-converter method `String` not `ToString`." A method named `Read` with a different signature misleads every reader who knows `io.Reader`.
- **Initialisms keep one case**: `URL` or `url`, never `Url`. `appID`, `ServeHTTP`. **Protocol Buffers compiler output is exempt**, which is why `UserId` from protoc sits next to hand-written `UserID` in many Go codebases.

  | English | Exported | Unexported | Wrong |
  |---|---|---|---|
  | XML API | `XMLAPI` | `xmlAPI` | `XmlApi` |
  | iOS | `IOS` | `iOS` | `Ios` |
  | gRPC | `GRPC` | `gRPC` | `Grpc` |
  | DDoS | `DDoS` | `ddos` | `DDOS` |
  | ID | `ID` | `id` | `Id` |
  | DB | `DB` | `db` | `Db` |

- Receivers: one or two letters, never `self` or `me`, the same across all methods of a type.
- Scope rule: "the further from its declaration that a name is used, the more descriptive the name must be."
- Constants are MixedCaps, never `MAX_LENGTH` or `kMax`. A constant name "must not be a derivative of" its value (`Twelve = 12` is wrong).
- Errors: types `FooError`, sentinel values `ErrFoo`. Error strings lowercase, no trailing punctuation.
- Gerrand's three qualities of a good name: "Consistent (easy to guess), Short, Accurate."
- **The Google Go Style Guide ranks its principles in order: Clarity, Simplicity, Concision, Maintainability, Consistency** [V]. Consistency is the tiebreaker, not the first rule. See Schools of thought.

**File names carry build semantics in Go** [V, cmd/go]:
- `*_GOOS.go`, `*_GOARCH.go`, `*_GOOS_GOARCH.go` are build constraints. A file named `config_windows.go` is excluded from Linux builds without any `//go:build` line.
- "Files whose names begin with '_' ... or '.' are ignored."
- `*_test.go` files are test-only.
- Generated files carry a line matching `^// Code generated .* DO NOT EDIT\.$`.

`encoding/json` [V]: v1 `Unmarshal` accepts "a case-insensitive match" of keys. v2 is case-sensitive unless `MatchCaseInsensitiveNames` is set. Untagged exported fields marshal under the Go name (`"UserID"`). **A Go v1 service hides case mismatches that every other consumer of the same payload will hit.**

### Swift

Swift API Design Guidelines [V]:
- Clarity at the point of use. Omit needless words. **Name by role, not type** (`supplier`, not `widgetFactory`). Compensate for weak type information (`addObserver(_:forKeyPath:)`).
- Factory methods begin with `make`.
- Mutating / non-mutating pairs: `sort` / `sorted`, `append` / `appending` (`-ed` when grammatical, `-ing` when the verb takes a direct object). For noun-named operations: `formUnion` / `union`.
- Booleans read as assertions (`isEmpty`, `intersects`).
- Protocols are nouns when they say what something *is* (`Collection`), and `-able` / `-ible` / `-ing` when they describe a capability (`Equatable`).
- **Acronyms are uniformly cased by position**: `utf8Bytes`, `isRepresentableAsASCII`, `userSMTPServer`.
- Value-preserving conversions omit the first argument label (`Int64(x)`). A prepositional phrase gets a label (`removeBoxes(havingLength:)`).
- **SE-0005** (implemented in Swift 3.0) [V]: the Clang importer renames Objective-C APIs mechanically, pruning redundant type names, and `NS_SWIFT_NAME` / `swift_name` overrides it. One source produces two idiomatic surfaces. A rename in the Objective-C header changes the Swift name in ways the header author may not predict [I].

### Python

PEP 8 [V]:
- "Consistency within a project is more important. Consistency within one module or function is the most important." And: "know when to be inconsistent."
- Public names reflect usage, not implementation.
- **"HTTPServerError is better than HttpServerError."** Acronyms stay uppercase in CapWords.
- Avoid `l`, `O`, `I` as single-character names.
- Modules: short, lowercase, underscores allowed. Packages: lowercase, underscores discouraged.
- Exceptions take an `Error` suffix when they are errors.
- mixedCase is allowed only where it already prevails (the `threading` module).
- `class_` to avoid a keyword, `_internal` for non-public, `__mangled` for name mangling, `__dunder__` reserved for the language.

Distribution names versus import names [V, PyPA name normalization]: the distribution name normalizes by `re.sub(r"[-_.]+", "-", name).lower()`. The import name is a separate thing (`scikit-learn` installs `sklearn`) [I]. That gap is a typosquatting surface (route supply chain to `security`).

### .NET and Kotlin

.NET Framework Design Guidelines (Cwalina and Abrams. The 2nd edition is online with an out-of-date warning. The 3rd edition, 2020, is print only.) [V]:
- **Two-letter acronyms all caps** (`IOStream`, `ioStream` in camelCase). **Three or more letters are PascalCase** (`HtmlTag`).
- Closed-compound spellings are specified, and several surprise people:

  | Use | Not |
  |---|---|
  | `Callback` | `CallBack` |
  | `Canceled` | `Cancelled` |
  | `Email` | `EMail` |
  | `Endpoint` | `EndPoint` |
  | `FileName` | `Filename` |
  | `Hashtable` | `HashTable` |
  | **`Id`** | `ID` |
  | `Indexes` | `Indices` |
  | `LogOn` / `LogOff` | `LogIn` / `LogOut` |
  | `Metadata` | `MetaData` |
  | **`Ok`** | `OK` |
  | `SignIn` / `SignOut` | `SignOn` / `SignOff` |
  | `UserName` | `Username` |
  | `Writable` | `Writeable` |

- "Names cannot differ by case alone," because Visual Basic and other CLR languages are case-insensitive.
- No underscores, no Hungarian, no abbreviations (`GetWindow`, not `GetWin`). Use CLR type names in method names (`ToInt64`, not `ToLong`), so the name means the same in every CLR language.
- New versions of an API: a suffix that sorts next to the old one. No `Ex`. Use `64` only when a 32-bit version exists.
- Interfaces take the `I` prefix. A class / interface pair differs only by the `I`. No `C` prefix on classes.
- Generic type parameters `TSession` style.
- Required suffixes: `Attribute`, `EventArgs`, `EventHandler`, `Exception`, `Collection`, `Dictionary`, `Stream`. `Callback` for delegates, never `Delegate`.
- Enums: singular names, plural for flags enums. No `Enum` or `Flags` suffix. **No prefixes on enum values.**
- Booleans: affirmative phrases (`CanSeek`, not `CantSeek`). `Is` / `Can` / `Has` only "where it adds value."
- Collection properties are plural, with no `List` suffix. Do not have a property and a `Get` method with the same name.
- Events use tense: `Closing` (before), `Closed` (after). No `Before` / `After` prefixes.
- **Task-based Asynchronous Pattern** (page revised 2026-04-17) [V]: an `Async` suffix on methods that return awaitables. `TaskAsync` if an event-based `XAsync` already exists. A method that starts work but returns no awaitable is `Begin` / `Start`. "Don't append `Async` to synchronous methods." Parameter names `cancellationToken` and `progress`.

Kotlin coding conventions [V]:
- A factory function may share its type's name (`fun Foo(): Foo`). `@Composable` functions that return `Unit` are PascalCase.
- Backtick test names (`` `returns empty list when input is empty` ``) are allowed in tests, but not on Android below API 30.
- `const val` and deeply immutable top-level `val` are `SCREAMING_SNAKE`. Backing properties use a leading underscore (`_elementList`).
- "Avoid Manager, Wrapper" and similar meaningless suffixes.
- Acronyms follow the .NET rule (`IOStream`, `XmlFormatter`).
- File names: `ProcessDeclarations.kt`, no `Util`. Multiplatform files take platform suffixes (`Platform.jvm.kt`).

### Java and JavaBeans

Google Java Style [V]:
- **Camel case is defined as an algorithm**: lowercase the phrase, split into words, capitalize each word's first letter. "XML HTTP request" becomes `XmlHttpRequest`. "new customer ID" becomes `newCustomerId`. This is the only definition that is deterministic.
- A constant is a `static final` field whose content is deeply immutable. That definition is semantic, so a linter cannot check it [I].
- JUnit test methods may use underscores (`transferMoney_deductsFromSource`).
- No `mName`, `s_name`, `kName`.
- The JDK itself breaks the rule (`HttpURLConnection`) [I].

**JavaBeans makes `get` / `set` / `is` a framework contract, not a style** [V, `java.beans.Introspector`]. `Introspector.decapitalize` leaves a name unchanged "when ... both the first and second characters are upper case": `getURL()` yields the property `URL`, while `getUrl()` yields `url`. Jackson computes property names by its own rules, which differ in some cases [U]. The same getter can produce two different JSON keys depending on which library reads it.

### TypeScript and JavaScript

- Google TypeScript Style Guide [V]: "`loadHttpUrl`, not `loadHTTPURL`, unless required by a platform name (e.g. `XMLHttpRequest`)." No `_` prefix or suffix. "Do not mark interfaces specially (`IMyInterface`) ... unless it's idiomatic in its environment." A `$` suffix on Observables is acceptable if used consistently. `CONSTANT_CASE` means "intended to not be changed." "Do not abbreviate by deleting letters within a word."
- **The TypeScript contributor guidelines are commonly mis-cited** as "the TypeScript team says no `I` prefix." The page says it is "NOT a prescriptive guideline for the TypeScript community" [V]. It governs the compiler's own source.
- Airbnb JavaScript Style Guide [V]: acronyms **all uppercase or all lowercase** (`HTTPRequests`, `SMSContainer`). That conflicts with Google. A file's name matches its default export exactly. Its ban on leading underscores rests on "JavaScript does not have the concept of privacy," which is stale since ES2022 `#private` fields [I].
- React [V, react.dev]: event handler props are `on` + event (`onClick`). Handler implementations are `handle` + event (`handleClick`). **`on*` is the contract, `handle*` is the implementation.**

### C and C++

- Google C++ [V]: files `my_useful_class.cc`. Functions PascalCase (`AddTableEntry`). Accessors `count()` / `set_count()`. Data members `table_name_`. Constants and enumerators `kName`. Macros `UPPER_SNAKE`.
- Linux kernel [V]: "C is a Spartan language." `tmp` is fine for a local. A global named `foo` "is a shooting offense": write `count_active_users()`. Hungarian is "asinine." No `typedef` for structs or pointers. Inclusive terminology (primary / secondary, denylist / allowlist).
- **Reserved names** [V, POSIX.1-2024 §2.2.2 and cppreference]:
  - Any identifier beginning with `_` followed by an uppercase letter, or containing `__`, is reserved everywhere. Declaring one is undefined behavior in ISO C.
  - External identifiers beginning with `_` are reserved.
  - POSIX reserves the `_t` suffix for any header, and prefixes including `str[a-z]`, `mem`, `wcs`, `is`, `to`, `E[0-9A-Z]`, `SIG_`, `LC_`.
  - So include guards like `_FOO_H_` are violations, and a user typedef `foo_t` is technically in the POSIX namespace. C23 adds "potentially reserved" identifiers.

### Ruby, Rails, Elixir, OCaml

- Ruby Style Guide [V]: `CapitalCase` with acronyms kept uppercase (`SomeXML`). **Predicates end in `?` and never use `is_`** (`tall?`). A `!` method only when a safe twin exists. `some_var1`, not `some_var_1`.
- **Zeitwerk** (Rails' autoloader) [V]: the default inflector camelizes `html_parser` to `HtmlParser`. A class named `HTMLParser` (the Ruby guide's spelling) in `html_parser.rb` fails to autoload unless an explicit inflection is configured. **The style guide and the autoloader disagree, and the autoloader wins at runtime.**
- Rails Active Record [V]: class `LineItem` maps to table `line_items`. `Person` maps to `people`. Foreign keys are `<singular_table>_id`. **Reserved columns**: `created_at`, `updated_at`, `lock_version`, `type` (turns on single-table inheritance), `*_type`, `*_count`. A column named `type` added for a domain reason makes Active Record try to load a subclass named by each row's value [V reserved, U on the exact exception name].
- Elixir naming conventions [V, elixir.hexdocs.pm/naming-conventions.html, moved from hexdocs.pm/elixir]: acronyms stay uppercase in module names (`ExUnit.CaptureIO`). A trailing `!` means the function raises. A trailing `?` means it returns a boolean, but guards use an `is_` prefix. **`size` is O(1), `length` is O(n).** `get` returns a default, `fetch` returns `:error`, `fetch!` raises.
- OCaml `List` [V]: the safe variants carry the mark (`find_opt`, `nth_opt`, `assoc_opt`, since 4.05). The unmarked defaults raise.
- **Whichever variant came later gets the mark** [I]. Elixir marks the raising variant. OCaml marks the safe variant. A reviewer must learn each ecosystem's direction, and must not import one ecosystem's marker into another.
- `!` means different things across languages: Ruby (dangerous variant), Elixir (raises) [V]. Scheme (mutation), Clojure (side effect) [U].

### Protocol Buffers, GraphQL, REST APIs

- Protocol Buffers style [V, protobuf.dev]:
  - Files `lower_snake_case.proto`. Messages `TitleCase`. Fields `snake_case`. Repeated fields plural.
  - Enum values `UPPER_SNAKE`, prefixed with the enum name, and the zero value ends in `_UNSPECIFIED` or `_UNKNOWN`.
  - `GetDnsRequest`, not `GetDNSRequest`. This conflicts with Go's initialism rule, which is why Go exempts protoc output.
  - `XYZ_2` should be `XYZ2` or `XYZ_V2` "to prevent collisions across language transformations."
  - No `has_`, `get_`, `set_`, `clear_` field prefixes and no `_value` suffix, because the generated accessors collide.
  - JSON mapping: fields become lowerCamelCase, `json_name` overrides, and "Parsers accept both" spellings.
  - **A field rename is safe on the binary wire (tags, not names) and breaking in JSON** [I].
- GraphQL (October 2021 spec) [V]: the spec mandates no casing. It defines the `Name` grammar, reserves `__` for introspection, and recommends only that enum values be "all caps." camelCase fields and PascalCase types are community convention [U].
- Google API Improvement Proposals [V]:
  - **AIP-122**: collection IDs "must be plural" and "must be in camelCase." This contradicts common kebab-case URL advice. User-supplied resource IDs follow RFC 1034 and may contain hyphens.
  - **AIP-140**: no prepositions in field names (`error_reason`, not `reason_for_error`). Adjectives first (`collected_items`). Allowed abbreviations: `config`, `id`, `info`, `spec`, `stats`. Unit suffixes (`distance_km`, `width_px`). **Booleans omit `is`** (`disabled`, not `is_disabled`). `uri` versus `url` used correctly. `display_name` and `title` have fixed meanings.
  - **AIP-142**: timestamps end in `_time` in imperative form: **`create_time`, not `created_time`**. Durations end in `_duration`. Offsets in `_offset`. Civil dates in `_date`. Legacy integer timestamps carry the unit (`send_time_millis`). This conflicts with Rails' `created_at`.

### SQL

- sqlstyle.guide (Simon Holywell, compatible with Joe Celko's SQL Programming Style) [V]:
  - Tables: "a collective name or, less ideally, a plural form" (`staff` over `employees`). No `tbl` prefix.
  - Never give a table the same name as one of its columns. Columns are singular.
  - **"Avoid simply using id"** as the primary key name. This conflicts directly with Rails.
  - Stored procedures contain a verb. No `sp_` prefix.
  - Standard suffixes: `_id`, `_status`, `_total`, `_num`, `_name`, `_seq`, `_date`, `_tally`, `_size`, `_addr`.
- PostgreSQL lexical structure [V]:
  - Unquoted identifiers fold to lowercase. That is "incompatible with the SQL standard," which folds to uppercase.
  - A quoted identifier is case-sensitive. A column created as `"userId"` must be quoted in every query forever. An unquoted `userId` in a query refers to `userid`, which does not exist.
  - **Identifiers are limited to 63 bytes, and longer names are truncated silently.** Two long index or constraint names that share their first 63 bytes collide.

### CSS

- BEM [V, bem.info]: the original Yandex scheme is `block-name__elem-name_mod-name_mod-val`, with a **single underscore** before a modifier. The popular `block__elem--mod` form is the "Two Dashes" alternative scheme. "Elements of elements do not exist": `block__elem1__elem2` is wrong. Mixing the two modifier schemes in one codebase is a consistency finding.

---

## Acronym casing: three camps

The most frequent cross-ecosystem conflict [V for each row]:

| Camp | Used by | Examples |
|---|---|---|
| **All caps** | Go, PEP 8, Ruby, Airbnb JS, Swift, Elixir modules | `ID`, `HTTPServerError`, `SomeXML`, `HTTPRequests`, `userSMTPServer` |
| **As a word** | Google Java, Google TypeScript, Rust, Protocol Buffers, Biome `strictCase` (default on) | `XmlHttpRequest`, `Uuid`, `GetDnsRequest`, `HttpServer` |
| **Hybrid** (two letters caps, three+ as a word, `Id` / `Ok` as words) | .NET, Kotlin | `IOStream`, `HtmlTag`, `Id`, `Ok` |

The DOM mixes camps in one API (`XMLHttpRequest`, `getElementById`).

**Case conversion is lossy** [V-exp, Pydantic 2.13.5]:
- `userID` and `userId` both convert to `user_id`, which converts back to `userId`.
- `getURL` converts to `get_url` and back to `getUrl`.
- `foo2` converts to **`foo_2`**: Pydantic inserts an underscore before a digit, and Protocol Buffers warns that `foo_2` and `foo2` collide across language transformations.
- Jackson's `SNAKE_CASE` strategy turns `theWWW` into `the_www` [V, source].

Only the as-a-word camp survives a round trip through snake_case [I]. That is the strongest system-scale argument for it. The all-caps camp's argument is that acronyms are spelled correctly and grep for `URL` finds them. See Schools of thought.

---

## Names at boundaries

The highest-severity findings in this lens. At each boundary, ask: **what spelling does each side expect, who converts, and what does each decoder do with a key it does not recognize?**

### Serialization and decoders

What happens to a key with the wrong case:

| Decoder | Result | Marker |
|---|---|---|
| Go `encoding/json` v1 | binds anyway (case-insensitive) | [V] |
| Go `encoding/json` v2 | ignored, unless `MatchCaseInsensitiveNames` | [V] |
| serde (Rust) | ignored. An `Option` field becomes `None`. Unknown fields ignored unless `deny_unknown_fields` | [V] |
| Pydantic | ignored | [U] |
| Jackson with defaults | throws on unknown property | [U] |
| Jackson as Spring Boot configures it | dropped | [U] |
| TypeScript `JSON.parse` plus a type assertion | `undefined` at runtime, no error | [I] |
| Protocol Buffers JSON parser | accepts both spellings | [V] |

**The same payload is accepted, rejected, or silently dropped depending on the consumer.** A test against a Go v1 or protobuf consumer proves nothing about the other consumers.

Mapping tools and their traps:
- **serde** [V]: `rename_all` takes `lowercase`, `UPPERCASE`, `PascalCase`, `camelCase`, `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case`, `SCREAMING-KEBAB-CASE`. Serialize and deserialize can be set separately. `rename_all_fields` covers struct variants of an enum.
- **Pydantic** [V]: `validate_by_alias=True` but `serialize_by_alias=False` by default. The docs call this "notably inconsistent" and announce a change in V3. **A model with camelCase aliases parses camelCase and dumps snake_case.** A round trip through such a model changes the wire format **VOLATILE**.
- **Jackson** `PropertyNamingStrategies` (since 2.12, `UPPER_SNAKE_CASE` since 2.13) [V].
- **JavaBeans** introspection versus Jackson for the same getter (see Java above).

### HTTP, URLs, and environment variables

- **HTTP field names are case-insensitive** (RFC 9110) [V]. **HTTP/2 requires lowercase field names**: an uppercase field name makes the message malformed, and intermediaries must not forward it (RFC 9113) [V]. Code that looks up a header by exact case (`headers["X-Request-Id"]` on a plain map) works over HTTP/1.1 and breaks behind an HTTP/2 proxy that lowercased it [I].
- **URL path components are case-sensitive** (RFC 3986). Only the scheme and host compare case-insensitively [V]. That is the technical argument for lowercase paths: `/Users/42` and `/users/42` are different resources.
- **`X-` prefixes are deprecated** (RFC 6648, 2012) [V]. A prefix that states a status (`X-` for experimental, `V2`, `Legacy`, `New`) outlives the status (`x-gzip` survives decades later).
- **Environment variables** [V, POSIX ch. 8]: standard utilities use uppercase letters, digits, and underscores, and "the name space of environment variable names containing lowercase letters is reserved for applications." The universal `MY_APP_PORT` convention sits in the utilities' namespace. A collision is rare in practice, but a prefix per application is the mitigation [I]. Windows environment variables are case-insensitive [U].
- The Twelve-Factor App [V]: environment variables are "granular controls ... never grouped together as 'environments'." A `ENV=staging` variable that switches many behaviors is a naming of a mode, not a control.

### Kubernetes and infrastructure names

Kubernetes resource names follow RFC 1123 / RFC 1035 label rules: lowercase alphanumerics and hyphens, 63 or 253 characters depending on the kind [V]. camelCase names are rejected. **One concept routinely carries three spellings**: `UserService` in code, `user-service` as the Kubernetes Service, and `USER_SERVICE_*` as the injected environment variables [U on the injected form]. That is acceptable when the mapping is mechanical and documented. It is a finding when one of the three is spelled differently (`users-svc`).

### Telemetry

Prometheus and OpenTelemetry conflict directly [V]:
- **Prometheus**: base units in the name (seconds, bytes, and grams for mass), ratios instead of percentages, plural unit suffixes, `_total` on counters (`http_requests_total`).
- **OTel semantic conventions**: dotted snake_case namespaces, units in metadata rather than the name, "SHOULD NOT append `_total`," no plurals (`system.process.count`).

Translation between the two renames metrics [V conflict]. A dashboard or alert written against the pre-translation name goes empty silently [I]. Route OTel attribute design to `otel-instrumentation`. This file flags the rename hazard.

### Case-insensitive filesystems

[V-exp on macOS APFS with git 2.50.0, unless marked]
- **A plain `mv Foo.ts foo.ts` is invisible to git.** The disk holds `foo.ts`, the index keeps `Foo.ts`, and `git status` is empty. The commit contains no rename. Linux CI then fails to resolve `./foo`, or resolves a stale `Foo.ts`.
- **`git mv Foo.ts foo.ts` works** in git 2.50 [V-exp]. The version where single-step case-only `git mv` started to work was not verified.
- `core.ignoreCase` is probed and set automatically at clone and init [V, git-config man page].
- Creating a branch `feature-x` when `Feature-X` exists fails on macOS with "already exists" [V-exp]. Lowercase branch names avoid the class.
- **TypeScript `forceConsistentCasingInFileNames` does not catch index drift** [I]: after a plain `mv`, the disk name and the import agree, so the compiler is satisfied. Its default is now `true` when `strict` is on [V] **VOLATILE**.
- Jest's `__mocks__` folder name is case-sensitive [V].

### Names a tool reads

A rename of any of these changes behavior:
- Go `_GOOS` / `_GOARCH` / `_test` file suffixes and `_` / `.` file prefixes [V].
- Test-runner discovery: Go `TestXxx` functions, pytest `test_` prefixes [U on pytest defaults, configurable], Jest / Vitest `*.test.*` globs.
- Python module file names must be valid identifiers (Ruff `N999`) [V].
- Rails reserved columns and pluralization, Zeitwerk inflection [V].
- JavaBeans `get` / `set` / `is` [V].
- **Storybook CSF** [V]: "Storybook will always use the named export to determine the story ID and URL." Renaming the export breaks links, bookmarks, and visual-test baselines keyed by story ID. Change the display name with `name:` instead.
- Kotlin multiplatform file suffixes (`.jvm.kt`) [V].
- Generated-file suffixes (`*.pb.go`, `*_pb2.py`) that lint and coverage configs exclude by glob [I].

---

## Vocabulary: one term per concept

### The rule and its sources

- Robert Martin, *Clean Code* ch. 2 (written by Tim Ottinger, 2008) [V via a translation-repository copy]: **"Pick One Word per Concept"** -- `fetch`, `retrieve`, and `get` as equivalent methods in different classes, or `controller`, `manager`, and `driver` for one role, is a defect. **"Don't Pun"** -- `add` meaning "concatenate" in one class and "insert into a collection" in another.
- Eric Evans, *Domain-Driven Design*, via Fowler's bliki [V]: **Ubiquitous Language** (2006) is "a common, rigorous language between developers and users." **Bounded Context** (2014): "total unification of the domain model for a large system will not be feasible or cost-effective," with the example of "meter" meaning different things to different groups.
- Simplified Technical English: one word, one meaning, one term per concept [I -- same rule, documentation register].

The synthesis [I]: **one term per concept within a bounded context, and an explicit rename at the boundary between contexts** (the anti-corruption layer). A synonym inside one context is a finding. Two words for related concepts in two contexts is correct, if the translation point is explicit.

### Verb semantics: the implicit lexicon

Einar Høst and Bjarte Østvold, "Debugging Method Names" (ECOOP 2009) [V]: a large Java corpus defines an implicit lexicon of what verbs mean, and a method whose behavior deviates from its verb's typical behavior is a "naming bug." Their example: AspectJ's `containsField` returns the found `Field` rather than a boolean. The suggested name is `find`. Their caution: "syntactic uniformity helps reduce the cost of 'human parsing' of identifiers, but not the interpretation."

The working lexicon, assembled from Høst and Østvold, Go, Rust, and Elixir [V for each source, I for the merge]:

| Verb | Reader expects |
|---|---|
| `is`, `has`, `can`, `should`, `contains` | Returns a boolean. No side effects. |
| `get` | Cheap. Returns a value that exists. (Elixir: returns a default when absent.) |
| `find`, `lookup`, `try_get`, `*_opt` | May return absent. |
| `fetch`, `load`, `read` | May do IO. May fail. (Elixir `fetch`: returns `:error` when absent.) |
| `compute`, `calculate` | Expensive, pure. |
| `create`, `new`, `make`, `build` | Allocates a new thing. |
| `to_` / `as_` / `into_` | Conversion, with Rust's cost and ownership semantics. |
| `set`, `update` | Mutates. Returns nothing (Arnaoudova A3). |
| `ensure` | Idempotent: creates if missing. |
| `validate`, `check` | Returns a result, or throws. Not silent (Arnaoudova B2). |
| `size` (Elixir) | O(1). `length` is O(n). |

### Linguistic antipatterns

Venera Arnaoudova et al., the Linguistic Antipatterns catalog (CSMR 2013, EMSE 21(1) 2016) [V catalog, U papers]. Method antipatterns:

- **A1**: `get` does more than return a value.
- **A2**: `is` returns something other than a boolean.
- **A3**: `set` returns a value.
- **A4**: the name says one, the return type is many.
- **B1**: a comment documents a condition the code does not implement.
- **B2**: a validation method returns nothing and does not throw.
- **B3**: the name promises an object, the method returns `void`.
- **B4**: a predicate name, no return value.
- **B5**: a transform method (`toX`, `convert`) does not return the result.
- **B6**: the name says many, the return type is one.
- **B7**: `getX` does not return the attribute `x`.
- **C1** / **C2**: the name and the type, or the comment and the signature, use antonyms (`isEnabled` returning `disabled`).
- **D1, D2, E1, F1, F2**: the same mismatches for attributes (a plural name holding one item, a boolean-named field holding a number, and so on).

**Familiarity numbs readers to misleading names**: 69% of developers unfamiliar with the code judged linguistic antipatterns to be bad practice, against 51% of the code's own maintainers [U, via Peter Hilton's summary of the EMSE 2016 paper]. A team cannot reliably catch its own antipatterns in review, which is the argument for an outside lens.

---

## Specific naming domains

### Booleans

- Prefix styles conflict: `is` / `has` (Swift, Kotlin, the typescript-eslint example config), `?` suffix with no `is` (Ruby), `is` omitted (Google AIP-140), `Is` "where it adds value" (.NET) [V].
- **Polarity**: .NET wants affirmative names [V]. Simonyi: a flag should "describe the true state of the flag" [V]. Negated names (`notReady`, `disableCache`) produce double negatives at call sites (`if (!notReady)`). No empirical study on boolean negation in identifiers was found [gap].
- **Absent-value safety at boundaries** [I]: a boolean field in a payload or config decodes to `false` when absent. Name it so `false` is the safe state. `disabled: false` by default means a feature is **on** when the field is missing. Whether that is safe depends on the feature. The name decides the default behavior of every old client that does not send the field.

### Events and handlers

- React: `on*` props, `handle*` implementations [V].
- .NET: tense in the name (`Closing` before, `Closed` after) [V].
- **Redux** [V, Redux Style Guide]: "Model Actions as Events, Not Setters" (Priority B): `food/orderAdded`, not `setPizzasOrdered`. Action types use `domain/eventName` (Priority C). Avoid `SET_DATA`-style generic types. Selectors are `selectX`. Slice state keys do not end in `Reducer` (`users`, not `usersReducer`).
- **The Redux guide's own examples include `todos/addTodo`**, which is exactly what `createSlice` generates from a reducer named `addTodo` [V]. The tool that defines the convention generates imperative names. A Redux codebase using `createSlice` will have imperative action types unless reducers are named as events (`todoAdded`). Flag the inconsistency only when the project has chosen the event style.
- Analytics event naming: route to `web-analytics`.

### Async

- .NET: `Async` suffix, required [V].
- Node: the *synchronous* variant is marked (`readFileSync`), the async one is unmarked [I].
- Rust, Kotlin, Swift: no suffix. The type (`Future`, `suspend`, `async`) carries it [I].
- A finding: mixing conventions within one codebase, or an `Async` suffix on a synchronous method (.NET says "Don't") [V].

### Factories and constructors

Rust `new` / `with_*` / `from_*` [V]. Go `NewX`, or `New` when the package names the type [V]. Swift `make*` [V]. Kotlin factory functions named after the type [V]. *Effective Java*'s `from`, `of`, `valueOf`, `getInstance`, `newInstance` [U]. The semantic split that matters across all of them: `getInstance` may return a shared instance, `newInstance` / `create` must return a new one [U on the Java list, I on the principle].

### Collections

Plural names for collections (Protocol Buffers repeated fields, AIP, .NET properties) [V]. No `List` / `Array` suffix that encodes the representation (.NET) [V]. Arnaoudova A4, B6, D1, E1 catch singular / plural mismatches [V].

### Units and kinds

- **Inside a typed language, carry the unit in the type**: `Duration`, `Instant`, a `Meters` newtype. Rust's deprecation of `sleep_ms` is the canonical move [V].
- **At an untyped boundary, carry the unit in the name**: `timeout_ms`, `distance_km`, `width_px` (AIP-140), `send_time_millis` (AIP-142), `http_request_duration_seconds` (Prometheus) [V].
- **Mars Climate Orbiter** (Mishap Investigation Board Phase I report, 1999-11-10) [V, llis.nasa.gov/llis_lib/pdf/1009464main1_0641-mr.pdf]: the root cause was "Failure to use metric units in the coding of a ground software file, 'Small Forces'." The `SM_FORCES` output fed the angular momentum desaturation (AMD) file, which "was required to be in metric units per existing software interface documentation." The impulse values were in pound-force seconds instead of newton-seconds, so they were "low by a factor of 4.45 (1 pound force = 4.45 Newtons)." AMD events occurred 10 to 14 times more often than expected. The spacecraft was lost on 1999-09-23 on a trajectory about 170 km lower than planned.
- **Correction to the folk version**: the unit *was* written down, in an interface document. It was absent from the data, the field name, and any type. This is a boundary-units case, not "a badly named variable." The lesson for review: a unit in a spec that is not in the name or the type is not enforced. Treat popular secondary figures (a "57 km" periapsis) with care.

### Ranges

Simonyi's `First` / `Last` (closed) / `Lim` (half-open) [V]. A finding: `end` used for both a closed and a half-open bound in one codebase, or `last` used for a half-open limit [I].

### Tests and test doubles

- Roy Osherove (2005) [V]: `UnitOfWork_StateUnderTest_ExpectedBehavior` (`Parse_OnEmptyString_ExceptionThrown`). No `Test` prefix. Name obviously bad inputs `BAD_DATA` so their role is clear.
- Google Java allows underscores in JUnit method names. Kotlin allows backtick sentence names [V].
- Runner name patterns are load-bearing (see Names a tool reads).
- **Test doubles** (Gerard Meszaros, *xUnit Test Patterns*, via Fowler "Mocks Aren't Stubs," 2006) [V]: **dummy** (passed, never used), **fake** (a working shortcut implementation), **stub** (canned answers), **spy** (a stub that records calls), **mock** (pre-programmed with expectations it verifies). Framework vocabulary blurs these: a Jest "mock function" is a spy in Meszaros's terms [U]. Name doubles by their role (`fakeClock`, `stubPricing`, `spyMailer`), so a reader knows whether assertions run against it [I].

### Generated files, commits, branches

- Go generated-code marker regex and conventional suffixes (`*.pb.go`, `*_pb2.py`) [V].
- **Conventional Commits 1.0.0** [V]: `feat` correlates with a MINOR release and `fix` with a PATCH release. Every other type (`chore`, `docs`, `refactor`) comes from commitlint and the Angular convention, not the spec. A breaking change is marked with `!` or a `BREAKING CHANGE:` footer, which "MUST be uppercase." Everything else is case-insensitive.
- Branch names: lowercase avoids case-insensitive ref collisions [V-exp].

### Errors

Go `ErrFoo` / `FooError` [V]. Rust `ParseIntError` (verb-object-error) [V]. Python `Error` suffix (Ruff `N818`) [V]. .NET `Exception` suffix [V].

---

## Enforcement: what a linter can and cannot check

**A linter checks spelling. It cannot check meaning.** Høst and Østvold's point about syntax versus interpretation, Google Java's semantic definition of a constant, and the feature-frozen state of typescript-eslint's rule all say the same thing [V]. The synthesis [I]: **lint casing and file names, and review vocabulary.**

Current tools [V] **VOLATILE**:
- **typescript-eslint `naming-convention`**: "feature frozen." Formats include `strictCamelCase` and `StrictPascalCase`, which enforce `userId` over `userID`. Boolean-prefix rules need type information. Ban the `I` prefix with `{"regex": "^I[A-Z]", "match": false}`.
- **Biome `useNamingConvention`**: not in the recommended set. **`strictCase` defaults to true**, so `HTTPServer` must become `HttpServer`. `requireAscii` defaults to true.
- **eslint-plugin-unicorn `filename-case`**: defaults to kebab-case. Has a `camelCaseWithAcronyms` case, checks directories, ignores `index.*`, and is off in the `unopinionated` config.
- **Clippy**: style group has `enum_variant_names`, `module_inception`, `upper_case_acronyms` (only fully-capitalized names unless `upper-case-acronyms-aggressive`), `wrong_self_convention`, `disallowed_names`, `just_underscores_and_digits`. Pedantic has `struct_field_names`, `many_single_char_names`, `similar_names`. Restriction has **`module_name_repetitions`** (moved there from pedantic; older posts say pedantic), `min_ident_chars`, `mod_module_files`, `self_named_module_files`.
- **Ruff `N801`-`N818`, `N999`**. The import-alias rules (`N811`-`N814`, `N817` acronym imported as a variable) police names that cross a module boundary.
- **SwiftLint `identifier_name`**: enabled by default, minimum length 3 (warning) / 2 (error), maximum 40 / 60, excludes `id`.
- **Checkstyle**: `AbbreviationAsWordInName` and the Google-specific `GoogleMethodName` / `GoogleNonConstantFieldName` checks.

**Learned naming tools** (Allamanis et al. NATURALIZE, FSE 2014: 94% top-suggestion accuracy and 14 of 18 suggested patches accepted [U]. JSNice, 62% variable-name prediction [V via Feitelson]) enforce *local* convention. That improves consistency and also entrenches a wrong convention [I]. The same is true of an AI coding assistant completing names from surrounding code [I].

**Renames and blame**: `git blame --ignore-revs-file` and the `blame.ignoreRevsFile` config skip a mass-rename commit [V]. Put a whole-concept rename in its own commit and list it there.

---

## Evidence: what the studies show and do not show

| Study | Finding | Limits |
|---|---|---|
| Hofmeister, Siegmund, Holt (SANER 2017) [V] | 72 professional C# developers: full words 19% faster for finding semantic defects than letters or abbreviations. **No difference between single letters and abbreviations.** Syntax-error finding unaffected. | 15-line snippets, online, 33% of records survived filtering |
| Lawrie, Morrell, Feild, Binkley (ISSE 2007) [V] | 128 participants: full words best for comprehension, but often not significantly better than abbreviations. Fewer syllables help memory. Very long names overload it. | Task and population differ from Hofmeister. Unresolved. |
| Binkley et al. (EMSE 2013, five studies, 150 participants) [V] | camelCase more accurate but 13.5% slower in one task, more accurate and faster in another, worse in a prose-like task. Underscores hurt beginners in recall. No effect in the final comprehension study. "Beginners benefit from camel casing." | Mostly students. **Evidence conflicts by task. Neither style wins.** |
| Sharif and Maletic (ICPC 2010) [U] | Eye-tracking replication, underscores faster to recognize | Standalone paper not read |
| Feitelson et al. (IEEE TSE) [V] | 6.9% median agreement on names. Three-step model (concepts, words, construction). Teaching the model improved names 2:1. | -- |
| Arnaoudova et al. (EMSE 2016) [U] | 69% vs 51% perception gap between outsiders and maintainers | Figures from a secondary summary |
| Høst and Østvold (ECOOP 2009) [V] | Verb-behavior deviations are detectable naming bugs | Java corpus |
| Butler, Wermelinger, Yu, Sharp (WCRE 2009, CSMR 2010) [U] | Identifier-quality flaws correlate with static-analysis warnings | Correlation only |
| Allamanis et al. (2014, 2015) [U] | Naming is a large share of code review comments (secondary sources give 9% to one third) | Figures conflict between secondaries |
| Deissenboeck and Pizka (2005) [U] | In Eclipse, identifiers are 33% of tokens and 72% of characters | -- |
| Gorla, Benander, Benander (IEEE TSE 1990) [U] | Debugging effort lowest at 10-16 character names | One COBOL study, correlational |

**No study was found that treats cross-boundary naming mismatches as a defect source** [gap]. That part of this file rests on specs, docs, and local experiments. Say so when it matters.

---

## Schools of thought (preserve disagreement)

These are unresolved. State each side at full strength. Do not average them.

### How much consistency counts

- **Consistency first.** PEP 8 ("Consistency within one module or function is the most important"), Simonyi's same-text target, Gerrand ("Consistent (easy to guess)" is first of three), Kernighan and Pike ("Do the same thing the same way everywhere"), and Feitelson's 6.9% agreement figure, which says guessability exists only where a convention creates it. A consistent wrong convention is still guessable, and guessability is what names are for.
- **Consistency last.** The Google Go Style Guide ranks Clarity, Simplicity, Concision, and Maintainability above Consistency [V]. Argument: consistency with a bad pattern spreads the bad pattern, and learned completion (and AI assistants) amplify whatever is locally common. Consistency is the tiebreaker when the other values do not decide.
- **When each is right**: consistency wins for casing, acronym style, file names, and verb lexicon, where the cost of variation is guessability and grep. Clarity wins when the existing convention is actively misleading (a `get` that does IO), because consistency with a lie compounds it.

### Fix as you go versus avoid churn

- **Boy Scout rule**: fix a bad name when you touch the code, or names never improve.
- **Against**: a half-migrated rename creates two words for one concept, which is worse than one wrong word. It also adds blame noise, merge conflicts, and Hyrum's-Law breakage at boundaries.
- A middle position [I]: rename a whole concept within one boundary in one dedicated commit (listed in `.git-blame-ignore-revs`), or do not rename it. This is a position, not a resolution.

### Short names versus long names

- **Short**: Pike, Go ("Short" is one of Gerrand's three qualities), the Linux kernel, Kernighan and Pike, and Lawrie's finding that abbreviations often comprehend as well as words. Argument: in a small scope, a long name is noise that hides structure, and the package or type qualifier (`bufio.Reader`) is part of the name.
- **Long**: *Clean Code*, .NET (no abbreviations), Swift (clarity at the point of use), SwiftLint's length minimum, Airbnb, McConnell citing Gorla, and Hofmeister's 19%. Argument: names are read far more than written, and abbreviations do not survive a change of reader.
- **Both agree** that length should grow with scope. They disagree on how steeply, and on whether a qualifier counts. A project rule (the user's own "no single-letter names except loop indices and math" is one) decides it locally, and project rules win.

### The `I` prefix on interfaces

- **For**: .NET Framework Design Guidelines. Argument: in C#, a class and an interface share syntax at the use site, and the `I` tells the reader which one they hold. The convention is universal in .NET, so omitting it is the surprise.
- **Against**: *Clean Code* (which prefers marking the implementation, `ShapeFactoryImp`), Google TypeScript ("unless it's idiomatic in its environment"), Java, Go, Rust, Swift. Argument: callers should not care, and the prefix is Hungarian notation for a type category.
- A side debate: `FooImpl` versus naming the implementation by what distinguishes it (`InMemoryFooStore`, `PostgresFooStore`). The second says something. `Impl` says only that an interface exists.

### The `get` prefix

- **Keep it**: Java, where JavaBeans makes it a framework contract [V], and .NET, which has explicit rules about `Get` methods versus properties.
- **Drop it**: Go and Rust. A field-like accessor reads as a noun.
- **Elixir** gives `get` a specific meaning (returns a default when absent) [V].
- **Common ground**: `get` must be cheap (Arnaoudova A1, Go's `Compute` / `Fetch` rule).

### Acronym casing

See the three-camp table. **All caps**: correct spelling, greppable, matches how people write the word. **As a word**: visible word boundaries in `XMLHTTPRequest`-type collisions, and lossless conversion to and from snake_case. **Hybrid**: a .NET compromise that is consistent within .NET and surprising outside it.

### Plural versus singular table names

- **Collective or plural**: Celko, Holywell's sqlstyle.guide ("a collective name or, less ideally, a plural form"), Rails (plural tables). A table holds a set.
- **Singular**: an ORM-friendly camp. A row is one entity, the class is singular, and irregular plurals (`person` / `people`, `index` / `indices`) complicate generated mappings.
- Both camps agree with Holywell's rule that a table must not share a name with one of its columns.

### Lint versus reviewer judgment

- **Lint**: ends bikeshedding, applies uniformly, costs nothing per review.
- **Judgment**: a linter checks syntax, not meaning (Høst and Østvold, Google Java's constant definition, the feature-frozen typescript-eslint rule).
- The common split [I]: lint casing, file names, and reserved-name rules. Review vocabulary, verb semantics, and boundary spellings.

### Naming by what versus naming by role

- **By role**: Swift ("name by role, not type": `supplier`, not `widgetFactory`).
- **By representation**: Systems Hungarian. Dead.
- **By domain term**: DDD's ubiquitous language, against pattern suffixes (`Factory`, `Manager`, `Repository`, `Service`). Kotlin: "avoid Manager, Wrapper."
- **By pattern**: the Gang of Four tradition, where `FooVisitor` and `FooFactory` tell the reader the mechanism immediately.
- Simonyi adds a minority position: do not use generic English words ("color") for program-specific types, which conflicts with DDD's use of domain words. Unresolved.

---

## Anti-pattern catalog

Each entry: the pattern, the trigger, the consequence, the fix.

### Boundary crossings

- **Case mismatch at a JSON boundary.** Trigger: producer emits `userID`, consumer expects `userId` (or snake_case). Consequence: silently `undefined` / `None` / dropped in most decoders, hidden by Go v1 and protobuf JSON. Fix: one spelling per boundary, a schema (OpenAPI, JSON Schema, protobuf), and `deny_unknown_fields` or equivalent in at least one consumer's tests.
- **Asymmetric alias config.** Trigger: Pydantic default `serialize_by_alias=False` with camelCase aliases. Consequence: the service parses camelCase and returns snake_case. Fix: set both explicitly.
- **Lossy round trip.** Trigger: `userID` or `foo2` through a snake_case converter. Consequence: the name comes back different, or collides with another field. Fix: as-a-word acronyms at converted boundaries, or explicit per-field names.
- **Quoted camelCase column in Postgres.** Trigger: an ORM or migration creates `"userId"`. Consequence: every hand-written query must quote it, and unquoted references fail. Fix: snake_case columns with ORM-level mapping.
- **Identifier over 63 bytes in Postgres.** Consequence: silent truncation, and collision between generated index or constraint names. Fix: shorter naming scheme for generated names.
- **Exact-case header lookup.** Consequence: breaks behind an HTTP/2 proxy. Fix: a case-insensitive header API.
- **Unitless number at a boundary.** Trigger: `timeout: 30` in JSON, YAML, env, or a column. Consequence: one side assumes seconds, the other milliseconds (Mars Climate Orbiter). Fix: unit suffix in the name, or a typed duration format (ISO 8601 `PT30S`).
- **Metric renamed by an exporter.** Consequence: dashboards and alerts go empty without error. Fix: pin the translated name, and alert on absent series.
- **Protobuf field rename.** Consequence: binary-compatible, JSON-breaking. Fix: treat a rename as a breaking change for JSON consumers, or set `json_name` to the old name.

### Vocabulary

- **Synonym rotation.** Trigger: `user`, `account`, `member`, `customer` for one entity, or `fetch` / `retrieve` / `get` for one action. Consequence: duplicated logic, search misses, and a new developer models them as different things. Fix: pick one per bounded context, write it in a glossary, rename in one commit.
- **Homonym.** Trigger: `order` meaning a purchase and a sort sequence in one context. Consequence: arguments about requirements that are about words. Fix: qualify one (`purchaseOrder`, `sortOrder`) or split the context.
- **Unmarked context boundary.** Trigger: two contexts use different words for related concepts, and the code converts implicitly in many places. Fix: one explicit translation point.
- **Status word in a name.** Trigger: `NewCheckout`, `LegacyApi`, `X-Custom-Header`, `v2` in a non-versioned concept. Consequence: outlives the status (RFC 6648). Fix: name the distinguishing property.

### Lying names

- **`get` that does IO or mutates** (A1). Consequence: called in a loop or a render path. Fix: `fetch` / `load`, or make it cheap.
- **`is` / `has` / `contains` returning non-boolean** (A2, `containsField`). Fix: `find`.
- **`set` returning a value** (A3), **`validate` that returns nothing** (B2), **`toX` that does not return** (B5). Fix: rename to the actual behavior, or change the behavior.
- **Singular / plural mismatch** (A4, B6, D1, E1). Fix: match the name to the cardinality.
- **Antonym mismatch** (C1, C2): `isEnabled` returning a `disabled` flag. Fix: one polarity.
- **Canonical name with a non-canonical signature**: a Go `String()` that takes arguments, a `Read` that does not match `io.Reader`. Fix: rename, or match the canonical signature.

### Framework-read names

- **Rails column named `type`.** Consequence: single-table inheritance turns on. Fix: `kind` or `<domain>_type`, or set `inheritance_column`.
- **Acronym class without a Zeitwerk inflection.** Consequence: autoload failure. Fix: add the inflection, or use `HtmlParser`.
- **Go file suffix collision.** Trigger: `handler_windows.go` meant as "the Windows-panel handler." Consequence: silently excluded from non-Windows builds. Fix: rename (`windows_panel_handler.go`).
- **Go file starting with `_`.** Consequence: ignored by the build. Fix: rename.
- **Storybook export rename.** Consequence: story ID and URL change, visual baselines orphaned. Fix: change `name:`, keep the export.
- **JavaBeans acronym getter.** Trigger: `getURL()`. Consequence: property `URL` in bean tools, possibly `url` in Jackson. Fix: as-a-word getters, or explicit `@JsonProperty`.
- **Reserved C identifiers.** Trigger: `_FOO_H_` guards, `my_type_t`. Consequence: undefined behavior in ISO C, a POSIX namespace collision. Fix: `FOO_H` guards, no `_t` suffix on user types.

### Consistency

- **Mixed acronym casing in one codebase.** Trigger: `userId` and `userID`, often from protoc output beside hand-written Go. Consequence: grep finds half the uses. Fix: pick one, enforce with the lint rule, and accept generated output as a documented exception.
- **Mixed BEM modifier schemes.** Fix: one scheme.
- **Mixed Angular pre-v20 suffixed and post-v20 unsuffixed files.** Route the file-layout part to `project-structure`.
- **Imported marker from another ecosystem.** Trigger: `fetch!` in a Ruby codebase meaning "raises," or `*_opt` in a language without the OCaml convention. Fix: use the host ecosystem's marker.
- **Boolean that is unsafe when absent.** Trigger: `enableFraudCheck` defaulting to false in old clients. Fix: name and default so the absent value is the safe value.

### Filesystem

- **Plain `mv` for a case-only rename on macOS or Windows.** Consequence: git records no rename, Linux CI fails. Fix: `git mv`, and a CI check on Linux. Consider a pre-commit check for case-colliding paths.
- **Branches differing only in case.** Fix: lowercase branch names.

---

## What is NOT a naming-conventions finding

- Whether a single name is clear at its point of use, with no convention or consistency angle. Route to `readability`.
- A name that follows the project's documented convention, even when the ecosystem canon differs. Project conventions win (`panel-contract.md`). The user's own rules (for example, no single-letter names outside loop indices and math values) are project conventions.
- A casing difference in generated code that the generator dictates and the project documents as an exception.
- Words a user sees. Route to `content-design`.
- Analytics event names. Route to `web-analytics`.
- OTel attribute naming beyond the Prometheus translation hazard. Route to `otel-instrumentation`.
- Directory meaning (`utils/` as a cohesion problem). Route to `project-structure`.
- A rename that would be correct but costs more than it returns at a public boundary. Name the cost as an insight, not a defect.

---

## Severity calibration (this domain)

- **blocker**: a name change at a boundary that silently drops or misreads data in production. A renamed JSON field with an old consumer that ignores unknown keys. A unitless number whose two sides disagree on the unit. A renamed metric behind a paging alert. A Rails column named `type` on a table with existing rows. A Go file suffix that excludes production code from a target platform.
- **major**: a new synonym for an existing domain concept inside one bounded context. A lying name on a hot path (`get` doing IO in a render or a loop). A public field or flag spelled against the boundary's convention, which will be expensive to rename later. A case-only rename done with plain `mv`. An exact-case header lookup on a path that crosses HTTP/2.
- **minor**: acronym casing inconsistent with the rest of the codebase. A boolean with negative polarity. A verb that is slightly off the lexicon (`retrieve` where the codebase says `fetch`). A test double named for the wrong Meszaros category.
- **nit**: ecosystem casing that a linter would catch and the project has not enabled. Interval naming that is correct but unconventional.
- **insight**: a proposal to adopt a lint rule, a glossary, or a unit-suffix policy. A whole-concept rename, with its blast radius named. A note that a convention the team follows is the minority position (for example, `I` prefixes in a TypeScript codebase).

Confidence: high when the finding cites a spec, a decoder's documented behavior, a tool's discovery rule, or a grep result showing the inconsistency. Medium when it argues from a canon the project has not adopted. Low when it depends on a volatile tool default not checked against the installed version.

---

## Authorities

| Source | Use it for |
|---|---|
| Charles Simonyi, "Hungarian Notation" (1999 reprint) | The original Apps Hungarian intent, interval qualifiers, the same-text target |
| Joel Spolsky, "Making Wrong Code Look Wrong" (2005) | Apps versus Systems Hungarian, kind prefixes |
| Rob Pike, "Notes on Programming in C" (1989) | Short names conditional on convention |
| Kernighan and Pike, *The Practice of Programming* (1999) | Scope-proportional names, uniformity |
| Tim Ottinger / Robert Martin, *Clean Code* ch. 2 (2008) | One word per concept, don't pun |
| Eric Evans, *Domain-Driven Design*; Fowler's bliki | Ubiquitous language, bounded contexts |
| Hyrum Wright, Hyrum's Law | Rename cost at observable boundaries |
| Rust API Guidelines, Clippy | Rust canon and enforcement |
| Effective Go, Go Code Review Comments, Gerrand (2014), Google Go Style Guide | Go canon, initialisms, consistency ranking |
| Swift API Design Guidelines, SE-0005 | Swift canon, role naming, importer renaming |
| PEP 8, Ruff | Python canon and enforcement |
| Cwalina and Abrams, *Framework Design Guidelines*; .NET TAP docs | .NET canon, compound-word table, async naming |
| Kotlin coding conventions | Kotlin canon |
| Google Java, TypeScript, C++ style guides | Per-language Google canon, the camel-case algorithm |
| Airbnb JavaScript Style Guide | The all-caps acronym JavaScript camp |
| Linux kernel coding style | C naming in the systems tradition |
| POSIX.1-2024 §2.2.2, ISO C via cppreference | Reserved identifiers |
| Ruby Style Guide, Zeitwerk, Rails guides | Ruby canon and the framework-read traps |
| Elixir naming conventions, OCaml stdlib | Marker direction (`!`, `_opt`), `size` versus `length` |
| Protocol Buffers style and JSON mapping | Proto naming, cross-language collisions |
| Google AIPs 122, 140, 142 | API field, boolean, and time naming |
| GraphQL spec | What GraphQL does and does not mandate |
| sqlstyle.guide, Celko, PostgreSQL docs | SQL naming, case folding, truncation |
| RFC 9110, 9113, 3986, 6648 | HTTP and URL case rules, `X-` deprecation |
| POSIX ch. 8, Twelve-Factor | Environment variable naming |
| serde, Pydantic, Jackson, JavaBeans Introspector docs | Boundary conversion behavior |
| BEM methodology | CSS naming |
| Meszaros via Fowler, Osherove | Test-double vocabulary, test names |
| Redux Style Guide, React docs, Storybook CSF | Frontend event, handler, and story naming |
| Conventional Commits 1.0.0 | Commit type semantics |
| Prometheus naming, OTel semantic conventions | Metric naming and their conflict |
| NASA MCO Mishap Investigation Board report (1999) | The boundary-units archetype, correctly told |
| Feitelson et al.; Furnas et al. | Name agreement rates, the naming model |
| Arnaoudova et al. | Linguistic antipattern catalog |
| Høst and Østvold | The implicit verb lexicon, naming bugs |
| Hofmeister et al.; Lawrie et al.; Binkley et al. | Empirical evidence on length, abbreviation, and casing |
| Allamanis et al., JSNice | Learned naming and its consistency effect |

---

## Process for the naming-conventions agent

1. **Identify the languages, frameworks, and boundaries in scope.** List every place a name leaves the language: serialization, database, environment, CLI flags, HTTP headers and paths, metrics, story IDs, file names read by tools.
2. **Read the project's documented conventions** and lint config. A documented convention outranks the ecosystem canon.
3. **Walk the boundaries first.** For each new or renamed name that crosses one: the spelling on each side, who converts, and what each decoder does with an unknown key.
4. **Walk vocabulary.** For each new domain noun or verb, grep for existing synonyms. Check the verb against the lexicon and the linguistic antipattern catalog.
5. **Walk framework-read names**: file suffixes and prefixes, reserved columns, autoloader inflections, bean getters, story exports, test-runner patterns.
6. **Walk units and kinds** on every numeric value at an untyped boundary.
7. **Walk consistency**: acronym casing, boolean polarity and prefix style, event and handler forms, async markers, ecosystem markers.
8. **Check casing against the ecosystem canon** only after the above, and only where the project has no stated rule.
9. **Check renames**: `git mv` for case-only changes, a dedicated commit for a whole-concept rename, and the boundary blast radius (route breaking-change analysis to `api-design`).
10. **Route** local clarity to `readability`, directory meaning to `project-structure`, public-contract evolution to `api-design`, analytics names to `web-analytics`, OTel names to `otel-instrumentation`.
11. **Stay read-only.**

---

## Changelog

**Source research**: `~/.claude/local/research-notes/naming-conventions-research.md` (about 16k words, claims tagged VERIFIED / V-exp / FOUND-UNVERIFIED / INFERRED, with a 20-item gaps list). Read it before a refresh.

- **2026-10-02** -- Initial version. Verified at source: Simonyi, Spolsky, Pike, Rust API Guidelines and Clippy source, Effective Go and the Google Go guide, cmd/go file-name rules, `encoding/json` v1 and v2, Swift guidelines and SE-0005, PEP 8, .NET FDG and TAP, Kotlin conventions, Google Java / TypeScript / C++ guides, TypeScript contributor wiki, Airbnb, JavaBeans Introspector, Linux kernel style, POSIX.1-2024 reserved names, Ruby guide, Zeitwerk, Rails reserved columns, Elixir and OCaml stdlib, Protocol Buffers style and JSON mapping, AIPs 122 / 140 / 142, GraphQL spec, sqlstyle.guide, PostgreSQL lexical rules, BEM, React, RFCs 9110 / 9113 / 3986 / 6648, POSIX environment variables, Twelve-Factor, Kubernetes names, PyPA name normalization, serde, Pydantic aliases, Jackson strategies, Storybook CSF, Conventional Commits, Prometheus and OTel naming, the NASA MCO board report, Feitelson, Arnaoudova's catalog, Høst and Østvold, Hofmeister, Lawrie, Binkley EMSE 2013, and the current lint tools. Local experiments: case conversion round trips (Pydantic 2.13.5) and case-only renames and branch collisions (macOS APFS, git 2.50.0). Known gaps: Sharif and Maletic 2010, Arnaoudova EMSE 2016 full text, Allamanis, Butler, and Ottinger's original page not read. Jackson defaults, Django and EF Core table naming, Windows env-var case, client HTTP/2 header handling, Go json/v2 release status, and Prometheus 3.0 UTF-8 names unverified. No study found on boolean negation or on cross-boundary naming mismatches as a defect source.
