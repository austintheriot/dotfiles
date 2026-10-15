---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-14
---

# React on the Server: Components, Functions, Caching, Hydration

A reference for the `react-server` subagent. The scope is the server side of React and the boundary between the two environments: React Server Components (RSC), the `'use client'` and `'use server'` directives, Server Functions (Server Actions), what crosses the boundary and how it is serialized, framework data caching and revalidation (Next.js App Router primarily), server rendering, streaming, prerendering, and hydration.

Out of scope, with the owner:

- Client hooks, effects, state, keys, refs, React Compiler, and client error boundaries: `react-components` (`~/.claude/rules/react-components.md`).
- Generic authentication design, session handling, and threat modeling beyond the React boundary: `security`. This file owns the React-specific facts that `security` would otherwise miss (a Server Function is a public endpoint, middleware is not an authorization boundary, a cache key is a data boundary).
- HTTP API design for route handlers as public APIs: `api-design`.
- CDN, hosting, and infrastructure config: `devops-infrastructure`.
- Bundle graph mechanics beyond the server/client split: `build-systems`.
- Cache-stampede and retry dynamics in general: `distsys-runtime`.
- Time-to-first-byte and streaming performance measured as cost: `performance`.

## Thesis

**An RSC app is one program that runs on two computers, and every line that crosses between them is a network boundary that looks like a function call.** `'use client'` is "a typed `<script>`" and `'use server'` is "a typed `fetch()`" (Dan Abramov, "What Does 'use client' Do?"). The ergonomics are the point, and they are also the hazard: an import statement creates a public POST endpoint, a prop sends data to every browser, a cached function's arguments decide who sees whose data, and a module's location in the import graph decides whether its secrets ship.

The operational question: **for each value and each call in this code, which computer runs it, who can reach it, and who else receives the result?**

### What bites most often

Empirical priority order, from the advisory record and the Next.js security guide:

1. **Server Functions without authorization inside them.** Every exported `'use server'` function is a public POST endpoint. Its ID is discoverable. A page guard does not cover it.
2. **Data over-shared across the boundary.** Raw database rows passed as props or returned from actions, secrets in client modules, server-only modules pulled into the client graph.
3. **Cache keys that omit the user.** A shared cache serves the first user's data to everyone.
4. **Middleware or proxy used as the authorization layer.** Five separate bypass classes in two years.
5. **RSC framework below the security floor.** Ten advisories against the Flight decoder in about ten months, two of them unauthenticated remote code execution (RCE).
6. **Revalidation mistakes.** Stale UI after a write, or the wrong primitive for read-your-writes.
7. **Control-flow throws swallowed.** `redirect()` or `notFound()` inside `try` / `catch`.
8. **Request waterfalls** in nested async Server Components.
9. **Hydration mismatches** from environment-dependent render output.

---

## Volatile surface

`last-verified` (see frontmatter, do not restate the date here). This domain rots faster than any other React surface. **Verify version floors and caching APIs at call time.** The agent has `Bash` (for `gh api`) and `WebSearch` for this reason.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Security advisories and the minimum safe versions | Extreme (weekly) | `gh api repos/facebook/react/security-advisories` , `gh api repos/vercel/next.js/security-advisories` |
| Next.js caching APIs, defaults, and flags | High (every 16.x minor changed something, Next 17 flips defaults) | https://nextjs.org/docs/app/getting-started/caching , https://nextjs.org/docs/app/api-reference/directives/use-cache , release posts |
| `unstable_` and `experimental.` names | High | the API reference page badges |
| React server APIs (`browser()`, `prerender` / `resume`, `cacheSignal`, taint status) | Medium (per React minor) | https://react.dev/blog , https://github.com/facebook/react/releases |
| RSC serialization rules | Low to medium | https://react.dev/reference/rsc/use-client , https://react.dev/reference/rsc/use-server |
| React Router RSC, TanStack Start, Waku, `@vitejs/plugin-rsc` status | High | each project's changelog and releases |
| Hydration fundamentals | Low | https://react.dev/reference/react-dom/client/hydrateRoot |

---

## 1. Versions and the security floor

### Current versions

**VOLATILE** (2026-10-14).

- React 19.3.1 (19.3.0 shipped 2026-09-09). Patch lines 19.2.9, 19.1.10, 19.0.9.
- Next.js 16.4.1. Majors and minors: 16.0 (2025-10-21), 16.1 (2025-12-18), 16.2 (2026-03-18), 16.3 (2026-08-03), 16.4 (2026-10-07). The 15.5.x backport line is maintained (15.5.28).
- React Router 8.4.0 (RSC still `unstable`). TanStack Start 1.0 (2026-10-13). Waku 1.0 release candidate. `@vitejs/plugin-rsc` 0.5.x.

### The security floor

**VOLATILE** (2026-10-14). **Re-check before stating.** Minimum safe versions today:

- `react-server-dom-webpack` / `-parcel` / `-turbopack`: 19.0.9, 19.1.10, 19.2.9, or 19.3.1.
- `next`: 15.5.28 or 16.4.1.

Next.js vendors `react-server-dom-*` inside `next/dist/compiled`. **Check the `next` version, not only `react`.** A lockfile with `react@19.3.1` and an old `next` is still exposed.

### The Flight decoder record

The Flight reply decoder is the code that parses Server Function arguments from the client. It runs on attacker-controlled POST bodies **before the server resolves which action was requested**, so the app's own action code does not matter. react.dev, 2025-12-03: "Even if your app does not implement any React Server Function endpoints it may still be vulnerable if your app supports React Server Components."

| Advisory | CVE | Type | Published | Fixed in |
|---|---|---|---|---|
| GHSA-fv66-9v8q-g76r | CVE-2025-55182 ("React2Shell") | Critical, unauthenticated RCE, CVSS 10 | 2025-12-03 | 19.0.1 / 19.1.2 / 19.2.1 |
| GHSA-925w-6v3x-g4j4 | CVE-2025-55183 | Server Function source exposure | 2025-12-11 | 19.0.2 / 19.1.3 / 19.2.2 |
| GHSA-2m3v-v2m8-q956 | CVE-2025-55184 | Denial of service (DoS) | 2025-12-11 | 19.0.2 / 19.1.3 / 19.2.2 |
| GHSA-7gmr-mq3h-m5h9 | CVE-2025-67779 | DoS, incomplete fix | 2025-12-12 | 19.0.3 / 19.1.4 / 19.2.3 |
| GHSA-83fc-fqcc-2hmg | CVE-2026-23864 | DoS | 2026-01-26 | 19.0.4 / 19.1.5 / 19.2.4 |
| GHSA-479c-33wc-g2pg | CVE-2026-23869 | DoS | 2026-04-08 | 19.0.5 / 19.1.6 / 19.2.5 |
| GHSA-rv78-f8rc-xrxh | CVE-2026-23870 | DoS | 2026-05-06 | 19.0.6 / 19.1.7 / 19.2.6 |
| GHSA-wx67-qw84-cm4g | CVE-2026-44907 | DoS | 2026-07-21 | 19.0.8 / 19.1.9 / 19.2.8 |
| GHSA-h3gv-c5rc-p453 | not yet assigned | DoS | 2026-10-14 | 19.0.9 / 19.1.10 / 19.2.9 / 19.3.1 |
| GHSA-xqc2-xf4c-x3h3 | not yet assigned | Critical, unauthenticated RCE | 2026-10-14 | 19.0.9 / 19.1.10 / 19.2.9 / 19.3.1 |

CVE-2025-55182 was reported by Lachlan Davidson through Meta's bug bounty. It affected Next.js, React Router, Waku, `@parcel/rsc`, `@vitejs/plugin-rsc`, and RedwoodSDK. CISA added it to the Known Exploited Vulnerabilities (KEV) catalog on 2025-12-05, with exploitation inside 48 hours (third-party reports, not verified against CISA directly). Next.js fixed versions for it: 15.0.5, 15.1.9, 15.2.6, 15.3.6, 15.4.8, 15.5.7, 16.0.7.

### Next.js advisories that change review rules

| CVE / advisory | What it proves | Fixed in |
|---|---|---|
| CVE-2025-29927 | `x-middleware-subrequest` header skipped middleware entirely | 15.2.3, 14.2.25, 13.5.9, 12.3.5 |
| CVE-2024-51479 | Pathname tricks bypassed middleware authorization | 14.2.15 |
| CVE-2026-44575, CVE-2026-45109 | `.rsc` and segment-prefetch URL variants bypassed middleware / proxy | 15.5.16, 16.2.5 |
| CVE-2026-44574 | Dynamic route parameter injection bypassed middleware / proxy | see advisory |
| CVE-2026-64642 | Turbopack with a single locale bypassed middleware / proxy | see advisory |
| CVE-2026-27978 | `Origin: null` (sandboxed iframe) bypassed the Server Action CSRF check | 16.1.7 |
| CVE-2026-64643 | Server Action and `'use cache'` endpoint IDs disclosed in public chunks | 15.5.21, 16.2.11 |
| CVE-2026-64649 | Server-side request forgery (SSRF) through Server Actions on custom servers with an unpinned Host header | 15.5.21, 16.2.11 |
| CVE-2026-64646 | Unbounded Server Action payload on the Edge runtime | 15.5.21, 16.2.11 |
| CVE-2026-44576 | RSC payloads poisoned a shared CDN cache that did not vary on RSC headers | 15.5.16, 16.2.5 |
| CVE-2026-103004 | A nested `'use cache'` call left a root param out of the outer key, leaking content across locales | 16.3.8 |
| CVE-2026-94544 | A pending `'use cache'` fill was shared between a Draft Mode request and a regular request, leaking unpublished content into prerenders | 16.3.x |
| GHSA-673q-mjm6-r5xr | Unauthenticated RCE in `'use cache'` functions that return typed arrays. "No known workaround", upgrade even without such functions | 16.4.1 (affects 16.0.0 to 16.4.0) |
| GHSA-p3m5-59mp-jv5r | CPU DoS in any App Router app with at least one Server Action | 15.5.28, 16.4.1 |

The lessons, which hold even after every listed version is patched:

1. **Middleware / proxy is not an authorization boundary.** Every bypass advisory gives the same workaround: authorize in the page, route, or action logic.
2. **An unguessable action ID is not access control.** CVE-2026-64643's text: "Never assume any authentication claims at the use cache or use server boundary. Always authenticate within the boundary."
3. **A `'use cache'` key is a data boundary.** The framework itself leaked across it twice.
4. **The RSC framework version is a security control**, not dependency hygiene.

---

## 2. The RSC model

### What `'use client'` marks

Source: https://react.dev/reference/rsc/use-client.

- It marks "the module and its transitive dependencies as client code". The boundary is on the "module dependency tree, not the render tree".
- Server or client is decided by **usage**. A component in a file with no directive is a Server Component when a server module renders it, and a Client Component when a client module imports it. The same file can be both, bundled twice.
- "When a 'use client' module is imported from another client-rendered module, the directive has no effect." Only entry points matter. A codebase with `'use client'` on every file has made every file a potential entry point and learned nothing about its boundary.
- "All code that is a part of the Client module sub-tree is sent to and run by the client." Not only components: constants, helpers, and any secret inlined in those modules.
- It must be the first statement in the file (comments allowed), in quotes, not backticks.

### Two module systems

Dan Abramov, "How Imports Work in RSC" (https://overreacted.io/how-imports-work-in-rsc/): "The two sides have two independent module systems." A module imported from both a server module and a client module is instantiated twice, with separate module state. A module-level singleton (a cache `Map`, a counter, a connection) is not shared between the two.

### Passing server content into client components

A Server Component can pass Server Components to a Client Component as `children` or as another JSX prop. The client component renders a slot it did not import, so the server subtree stays on the server. This is how a client-only shell (a modal, a tab set) wraps server-rendered content without pulling it into the client bundle. A Client Component that *imports* the server component instead turns it into a client component.

### What crosses the boundary: server to client props

Serializable (https://react.dev/reference/rsc/use-client):

- Primitives, including `bigint` and symbols registered with `Symbol.for`.
- `String`, `Array`, `Map`, `Set`, typed arrays, `ArrayBuffer`, `Date`, plain objects.
- Server Functions.
- JSX elements (server or client).
- Promises (unwrap in the client with `use()`).

Not serializable:

- Functions that are not Server Functions (an `onClick` from a Server Component).
- Class instances: ORM documents, `Decimal`, Luxon `DateTime`, Mongoose documents.
- Null-prototype objects (`Object.create(null)`, the output of some query-string parsers).
- Unregistered symbols. `URL` instances (listed as unsupported in the Next.js docs).

The non-obvious failure is the success case: **an ORM that returns plain objects serializes cleanly, so every column ships.** Password hashes, internal flags, other users' emails on a join. The data is in the RSC payload, visible in view-source and devtools, whether or not the component renders it.

### What crosses the boundary: Server Function arguments

The same list, plus `FormData`, minus JSX elements and event objects. Return values follow the server-to-client prop rules.

### Naming

react.dev: "Until September 2024, we referred to all Server Functions as 'Server Actions'." A Server Function passed to an `action` prop or called inside an action is a Server Action. The Next.js docs still say "Server Actions". This file uses "Server Function" except where quoting Next.js.

### Guards: `server-only`, `client-only`, and taint

- **`import 'server-only'`** at the top of a module makes any client import of it a build error. Next.js handles it internally (the npm package contents are not used). `@vitejs/plugin-rsc` validates it at build time. Dan: the failure "automatically propagates up the import chain", and these markers "don't put code on the backend". They guard modules, not values.
- **`server-only` does not stop a server module from returning a full row to a Server Component that passes it as a prop.** Modules are guarded by `server-only`, shapes by Data Transfer Objects (DTOs), values (experimentally) by taint.
- **Taint APIs** (`experimental_taintObjectReference`, `experimental_taintUniqueValue`) are still on the experimental channel only as of 19.3, not stable. Next.js enables them with `experimental.taint`. react.dev's pitfall: "the clone of a tainted object will create a new untainted object". `{ ...user }` is not tainted. Taint is a backstop, never the control. **VOLATILE.**
- **React Router RSC Framework Mode drops the built-in `.server` / `.client` module convention.** A Remix or React Router app migrated to RSC mode silently loses that guard. Use `import "server-only"` or `vite-env-only` `denyImports`.

---

## 3. Server Functions are public endpoints

### The facts

- **Every exported `'use server'` function is reachable by a direct POST.** Next.js docs: "Server Functions are reachable via direct POST requests, not just through your application's UI. Always verify authentication and authorization inside every Server Function."
- **A page-level auth check does not cover inline actions on that page.** Next.js data-security guide: the action "is a separate entry point and must verify the caller on its own". An action ID harvested from an admin's page works for any caller.
- **Arguments are attacker-controlled.** react.dev: "Arguments to Server Functions are fully client-controlled. For security, always treat them as untrusted input." TypeScript types are erased. `formData.get("qty") as number` is a string or null at runtime.
- **Authentication is not authorization.** A check that a session exists, without a check that this user owns this resource, is an insecure direct object reference (IDOR).
- **Never accept identity as an argument.** `updateProfile(userId, data)` lets any caller edit any profile. Derive the user from the session on the server.

### Next.js hardening, and its limits

- Action IDs are encrypted and non-deterministic, cached for at most 14 days, and regenerated each build. Unused actions are removed from the client bundle (since 15). Next.js 15 blog: "You should still treat Server Actions as public HTTP endpoints." CVE-2026-64643 showed the IDs leak anyway.
- **Closure encryption.** Values an inline action captures from component scope are encrypted with a per-build key (`NEXT_SERVER_ACTIONS_ENCRYPTION_KEY`, a base64 AES key of 16, 24, or 32 bytes, pins one key across instances). Docs: "We don't recommend relying on encryption alone." The ciphertext round-trips through the client, and a captured ciphertext can be replayed within the same build (inferred from the mechanism, not documented). Capture IDs, then re-fetch on the server.
- **CSRF check.** POST only, plus an Origin-versus-Host (or `X-Forwarded-Host`) comparison. `experimental.serverActions.allowedOrigins` matches the Origin host only. `*` matches exactly one label, `**` one or more (prefix only). Ports cannot be wildcarded. Docs: "A request that carries no `Origin` header at all is allowed through with a warning rather than rejected." The Origin check defends against cross-site browser requests. It is not authentication.
- Behind a proxy that rewrites Host without forwarding `X-Forwarded-Host`, actions fail with an origin mismatch, and teams "fix" it with a broad `allowedOrigins`. The right fix is to forward the header.
- `experimental.serverActions.bodySizeLimit` defaults to 1 MB, including multipart overhead.

### Runtime semantics that matter in review

- **Sequential dispatch.** Next.js docs: "The client currently dispatches and awaits them one at a time." A Server Function used for reads serializes those reads and queues them behind any pending mutation. react.dev: Server Functions "are not recommended for data fetching".
- **`redirect()` throws**, so code after it does not run. Revalidate before redirecting.
- Setting or deleting a cookie in an action re-renders the current page and its layouts.
- A Server Function called from `useEffect` is a POST per mount, two in development under StrictMode.
- **Return only what the UI needs.** Next.js: "Only return what the UI needs, not raw database records." `catch (e) { return { error: e.message } }` sends internal error text to the client, because returned values are not sanitized. Only thrown errors are reduced to a digest in production.
- `experimental.useOffline` (16.3) keeps an action pending through a network drop and completes it on reconnect. A non-idempotent action (charge, send) can then run minutes later than the user expects. Mutating actions need idempotency keys. **VOLATILE.**
- Next.js 16.2+ development logs print Server Function arguments. Treat those logs as containing personal data.

### The recommended structure

The Next.js data-security guide recommends a server-only **Data Access Layer (DAL)**: one module family, guarded with `server-only`, that does authentication, authorization, and DTO shaping, and is the only code that reads `process.env`. Server Components and Server Functions call the DAL. They do not query the database directly.

The guide's audit checklist, which this agent applies:

- DAL isolation.
- `'use client'` component props that are wider than needed.
- `'use server'` functions: arguments validated, user re-authorized, ownership checked, return values filtered.
- `[param]` route segments treated as user input.
- `proxy.ts` and `route.ts` given conventional penetration-test attention.

### Other frameworks

- **React Router** (v8, RSC `unstable`): after a Server Function runs, the route revalidates automatically. The public-endpoint property is the same.
- **TanStack Start**: server work is explicit through `createServerFn`. Each one is a public endpoint with the same rules. Validate input. (The validator method name changed across versions. Check the current docs.)

---

## 4. Data fetching on the server

### Waterfalls

Each `await` in an async Server Component delays its children. Nested components that each await their own data run one after another, and time to first byte is the sum of the latencies. Sebastian Markbåge: "the most common culprit by far is client-server waterfalls", and RSC moves them to the server, where they still exist.

Fixes:

- Start promises before awaiting them: `const userPromise = getUser(); const orgPromise = getOrg(); const [user, org] = await Promise.all([userPromise, orgPromise]);`.
- Preload through `cache` (call without awaiting in a parent).
- Push awaits down behind `<Suspense>` so the shell streams first.
- Pass the promise to a Client Component and unwrap it with `use()`.
- In a Next.js layout, pass the `params` promise down instead of awaiting it, so the layout stays in the static shell.

### `React.cache`

Source: https://react.dev/reference/react/cache.

- "For use in Server Components only." "React will invalidate the cache for all memoized functions for each server request." It is per-request deduplication, not a data cache.
- **Each `cache()` call creates a new memo.** Define it at module scope. `const getUser = cache(fetchUser)` inside a component shares nothing.
- Called outside a component, it "will still evaluate the function but not read or update the cache".
- Arguments are compared with `Object.is`. Pass primitives. An object argument built per call never hits.
- `cacheSignal()` (19.2, RSC only) aborts work when the cache lifetime ends.
- In Next.js, `React.cache` values set outside a `'use cache'` scope are not visible inside it. Pass data as arguments.

---

## 5. Next.js caching

### First: which model is the repo on?

Next.js 16.x has **two caching models in one major version**. Advice for one is wrong for the other. Check `next.config` before judging any caching code.

- **Previous model** (`cacheComponents` off): the `fetch` cache, `unstable_cache`, route segment config (`dynamic`, `revalidate`), `revalidatePath`, `revalidateTag`. Docs: https://nextjs.org/docs/app/guides/caching-without-cache-components.
- **Cache Components** (`cacheComponents: true`, plus `partialPrefetching: true` for the full model): `'use cache'`, `'use cache: private'`, `'use cache: remote'`, `cacheLife`, `cacheTag`, `updateTag`, `revalidateTag(tag, profile)`, `refresh()`, `connection()`. Partial Prerendering (PPR) is the default rendering. Runtime data must be inside `<Suspense>`.

**VOLATILE** (2026-10-14). The 16.4 post: Cache Components "will become the default in Next.js 17". New `create-next-app` apps have it on since 16.4.

### The history a reviewer needs

- **Next.js 15 (2024-10-21) reversed the defaults.** "`fetch` requests, `GET` Route Handlers, and client navigations are no longer cached by default." The client router cache stale time for page segments became 0 (`experimental.staleTimes.dynamic: 30` restores the old behavior). Calling `revalidateTag` or `revalidatePath` during render throws. Advice written for Next.js 13 and 14 ("fetch is cached by default, add `no-store`") is wrong for 15+.
- Markbåge, "Our Journey with Caching" (2024-10-24): "the developer experience suffered due to the caching defaults and controls we provided".
- **Next.js 16 removed and renamed:**
  - `middleware.ts` became `proxy.ts` (Node runtime, exported function `proxy`). `middleware.ts` is deprecated and kept for Edge only.
  - `experimental.dynamicIO` became `cacheComponents`. `experimental.ppr` and `export const experimental_ppr` were removed.
  - Synchronous `cookies()`, `headers()`, `draftMode()`, `params`, and `searchParams` were removed. They are async only.
  - `revalidateTag(tag)` with one argument is deprecated. It needs a profile.
  - Turbopack is the default bundler.
- **16.3**: "dynamic by default, with no hidden or implicit caching." `catchError` from `next/error`. Root params (`import { lang } from "next/root-params"`, Server Components only).
- **16.4**: `export const ensureStatic = "navigation" | "prefetch" | "shell"` fails the build when dynamic content leaks into that stage. `navigation()` and `prefetch()` from `next/cache`.

### `'use cache'`

Source: https://nextjs.org/docs/app/api-reference/directives/use-cache.

- **Cache key**: build ID (or `deploymentId`), function ID, serialized arguments, **variables captured by closure** (bound as arguments automatically), and an HMR hash in development. Only root params that the function actually reads join the key.
- **The key is the data boundary.** A function whose output depends on the user but whose arguments do not include the user serves the first caller's data to every caller until revalidation. The usual cause: the user is read through a side channel the framework does not see (an auth library that reads headers through AsyncLocalStorage, a module global). Pass the user or session ID as an argument, use `'use cache: private'`, or do not cache.
- **Forbidden inside**: `cookies()`, `headers()`, `searchParams`, `connection()`. The ban follows the call stack into helpers (error `next-request-in-use-cache`). Docs: "it can pass `next build` and fail under `next start`." A green build does not prove the scope is clean.
- **Awaiting runtime data inside** (a `cookies()` promise passed as a prop, through a closure, or through a shared `Map`) hangs the build for 50 seconds, then fails: "Filling a cache during prerender timed out, likely because request-specific arguments such as params, searchParams, cookies() or uncached data were used inside "use cache"".
- **Serialization**: arguments follow Server Function rules, return values follow client prop rules (JSX can be returned). `children` and Server Actions can pass through a cached component without affecting its key, if the component does not inspect them.
- **Storage**: the default in-memory LRU does not persist across requests on serverless and does persist when self-hosted (`cacheMaxMemorySize`). No entry survives a deploy, because the build ID is in the key.
- **Default profile** without `cacheLife`: stale 5 minutes, revalidate 15 minutes, expire never. The docs recommend explicit `cacheLife` in every scope. The client router enforces a 30-second minimum stale time.
- **A file-level `'use cache'` module imported from a Client Component makes its exports callable server endpoints**, "similar to a Server Function". It is an authorization surface with the same rules as section 3. CVE-2026-64643 names "use cache endpoints" as disclosable.
- **Caching an authorization decision** (`'use cache'` around `canEdit(userId, docId)`) keeps a revoked permission alive for the cache lifetime. Do not cache authorization, or tag it and invalidate on every permission change.

### `'use cache: private'` and `'use cache: remote'`

- **`private`** can read `cookies()`, `headers()`, and `searchParams`. It never stores results in a server cache. Results live in browser memory for the `stale` time only. It is excluded from the static shell. Stale must be at least 30 seconds for per-link prefetch, and at least 5 minutes to join the App Shell.
- **`remote`** uses a platform cache handler (Redis, KV). Every hit is a network round trip with a fee. It pays off only at a high hit rate.

### Revalidation

| Primitive | Semantics | Where |
|---|---|---|
| `updateTag(tag)` | Expire, then read fresh in the same request (read-your-writes) | Server Actions only |
| `revalidateTag(tag, profile)` | Stale-while-revalidate: serve stale, refresh in background | Server Actions, route handlers |
| `refresh()` | Re-render uncached data, cache untouched | Server Actions only |
| `revalidatePath(path)` | Invalidate a path | Server Actions, route handlers |

- **After a user's own write, use `updateTag`.** `revalidateTag("posts", "max")` followed by `redirect("/posts")` shows the user their stale list. They think the save failed and submit again.
- **A mutation with no revalidation** leaves the UI stale until the cache lifetime ends.
- **Revalidate before `redirect()`**, because `redirect()` throws.

### Time, randomness, and environment in prerendered routes

- `Math.random()`, `Date.now()`, and `crypto.randomUUID()` must run after `await connection()` inside `<Suspense>`, or inside a `'use cache'` scope (where every user shares one value until revalidation). `performance.now()` is exempt.
- A server environment variable read in a statically prerendered component is evaluated at build time and baked into the HTML and RSC payload. A secret rendered there is a secret in a CDN-cached file (inferred from the mechanism). Read runtime values after `await connection()`.
- `NEXT_PUBLIC_*` values are inlined at `next build` and frozen. One Docker image promoted across environments keeps the build environment's values. Dynamic lookups (`process.env[name]`) are not inlined.
- Next.js detects some crawlers by user agent and renders them dynamically, so a data source that exists only at build time gives those crawlers a 500.

### CDN in front of the App Router

A shared cache keyed on URL alone can serve an RSC payload for an HTML URL to later visitors (CVE-2026-44576). The CDN must vary on the RSC request headers or bypass RSC requests. See also `devops-infrastructure`.

---

## 6. Server rendering, streaming, and hydration

### Streaming and prerendering APIs

- `renderToPipeableStream` (Node) and `renderToReadableStream` (web). React 19.2 notes that Web Streams work in Node but "we still highly recommend using the Node Streams APIs". Next.js 16.3 moved App Router SSR to Node streams.
- `<Suspense>` boundaries are the streaming units. The shell streams first, and each boundary streams when its data resolves. A page with no Suspense boundary waits for its slowest await.
- `prerender` (`react-dom/static`) waits for every boundary to resolve and does not stream. It does not support `nonce`: a nonce baked into a prerender is insecure.
- React 19.2 Partial Prerendering: `prerender` returns `{ prelude, postponed }`, then `resume`, `resumeAndPrerender`, `resumeToPipeableStream`, or `resumeAndPrerenderToNodeStream`.
- React 19.2 batches server Suspense reveals briefly, and stops batching as load time nears 2.5 seconds.

### Hydration mismatches

react.dev (https://react.dev/reference/react-dom/client/hydrateRoot) lists the causes: whitespace around the root HTML, `typeof window !== "undefined"` checks in render, browser-only APIs such as `window.matchMedia` in render, and different data on server and client. Other common causes:

- `new Date()` or `Date.now()` rendered as text, and `toLocaleString()` with a server locale or time zone that differs from the browser's. Format with an explicit `locale` and `timeZone`. See also `i18n`.
- `Math.random()` IDs. Use `useId`.
- Invalid HTML nesting (`<div>` inside `<p>`, `<a>` inside `<a>`, `<tr>` outside `<tbody>`). The browser parser repairs it before hydration, so the DOM differs from React's tree.
- Browser extensions that inject attributes into `<html>` or `<body>`.
- A non-public environment variable read in a Client Component: defined during SSR, undefined in the browser.

Consequences, from react.dev: "There are no guarantees that attribute differences will be patched up in case of mismatches", and "in the worst case, event handlers can get attached to the wrong elements". Text mismatches are not patched. React may discard the server HTML for that boundary and render it on the client, which costs the SSR benefit.

### Fixes, and their limits

- **`suppressHydrationWarning` "only works one level deep"**, on the exact element (a timestamp, the `<html>` element that a theme script modifies). On a container it does nothing for its children.
- **The `isClient` effect pattern** (`useEffect(() => setMounted(true), [])`) "makes hydration slower because your components have to render twice". Applied to a large subtree, it removes that content from SSR and search indexing and adds a layout shift.
- **React 19.3 `use(browser())`** inside `<Suspense>` marks a subtree as browser-only: the server renders the fallback, the client renders the content, and no recoverable error is reported. The `onBrowserBailout` server option observes it. On 19.3+, it replaces the `isClient` pattern.
- For layout that differs by viewport, a CSS media query avoids the mismatch entirely.
- React 19.2 changed the `useId` prefix to `_r_` (it was `:r:` in 19.0 and `«r»` in 19.1). Snapshot tests and CSS selectors keyed on generated IDs break on upgrade.

---

## 7. Errors on the server

- **Expected errors are return values.** Next.js recommends returning them through `useActionState` and avoiding `try` / `catch` that throws for validation.
- **In production, errors forwarded from Server Components show "a generic message with an identifier"** in `error.digest`. UI that shows `error.message` works in development and shows a generic string in production. Do not branch on message text. Log and correlate the digest.
- **`redirect()`, `permanentRedirect()`, and `notFound()` throw control-flow errors.** So do request-time APIs and uncached fetches in a segment that must be static or is under PPR. A `try` / `catch` around them swallows the redirect, renders a 404 as a 200, or hides the dynamic bailout. Call them after the `try`, or call `unstable_rethrow(error)` as the first statement in the `catch`.
- **A hand-written class error boundary around routes catches those control-flow errors** and breaks redirects. Next.js 16.3's `catchError` (`next/error`) boundaries do not intercept them.
- **`error.tsx`** must be a Client Component. Its `retry` prop (stable in 16.3, `unstable_retry` in 16.2) refetches. `reset` only clears state. It does not wrap the `layout` or `template` of its own segment. Root layout errors need `global-error`, which renders its own `<html>` and `<body>`.
- `instrumentation.js` `onRequestError` (stable since 15) reports server errors with the context (Server Component, Server Action, route handler, proxy). See also `crash-and-release-health`.

---

## 8. Bundling boundary

- React publishes different entry points under the `react-server` export condition. Client-only APIs (`useState`, `useEffect`) are absent there, which is why a Server Component that calls them fails to build. Library authors use the condition to ship server-safe builds. (Mechanism partly verified. No primary doc page found.)
- **Barrel files pull server code into the client graph.** A Client Component that imports one helper from `lib/index.ts`, which re-exports the database module, puts the database module in the client graph. With `server-only`, that is a build error. Without it, it is silent bloat or an inlined secret (inferred). `optimizePackageImports` helps for third-party packages only.
- `next experimental-analyze` (16.1+) traces imports across the server-to-client boundary. Use it to find server code in client bundles.
- `serverExternalPackages` keeps a package out of the server bundle (Turbopack resolves transitive ones since 16.1).

---

## 9. Schools of thought

Each position is stated in its advocates' terms. None is adjudicated.

### A. Server-first is React's architecture

- **The React team and Next.js.** react.dev frames RSC as React's architecture since 19. Dan Abramov's 2025 essay series argues components that span two computers, "RPC at the module system level", and "One Roundtrip Per Navigation" (https://overreacted.io/react-for-two-computers/ , https://overreacted.io/jsx-over-the-wire/ , https://overreacted.io/impossible-components/ , https://overreacted.io/what-does-use-client-do/ , https://overreacted.io/one-roundtrip-per-navigation/). Markbåge: client-server waterfalls are "the most common culprit by far". Next.js 16.4: Cache Components is "the best choice for every Next.js app".
- **Kent C. Dodds** (https://www.epicreact.dev/react-server-components-the-future-of-ui): "I'm confident that React server components are the future", while still recommending Remix / React Router for now.

### B. Server Components should be optional

- **Tanner Linsley** (TanStack Start 1.0, 2026-10-13): Next.js made "a very big bet on React Server Components" that constrained "the React ecosystem's idea of an application". TanStack built RSC support, then "decided to stop using it on tanstack.com". Server work is explicit and opt-in through `createServerFn`.
- **React Router** (Brooks Lybrand, v8 announcement, 2026-06-17): "We don't force you into using Server Components, SSR, SSG, or any other acronym." RSC is still `unstable` 16 months after the first preview.
- **Michael Jackson and Ryan Florence** ("Wake up, Remix!", 2025-05-28): Remix 3 leaves React for a Preact fork, and "All packages must be designed with no expectation of static analysis", a direct rejection of bundler-integrated directives.
- I found no primary source for Ryan Carniato's or Theo Browne's positions on RSC. Do not attribute quotes to them from memory.

When each is right: the server-first camp is answering waterfalls and bundle size in content-heavy, data-heavy pages. The opt-in camp is answering highly interactive apps where most state is client state, and teams that want an explicit network boundary they can see in the code.

### C. Server Actions or explicit endpoints for mutations

- **For Server Actions**: one round trip returns data and UI, forms work before hydration (progressive enhancement), and the call site is typed ("'use server' is a typed fetch()").
- **For explicit endpoints**: the security record. Ten Flight-decoder advisories in ten months, action ID disclosure (CVE-2026-64643), and repeated Server Action DoS advisories (2025-01, 2026-07, 2026-10) show the RPC surface is large and implicit. Next.js itself hedges: "not recommended for data fetching", sequential dispatch, and route handlers for parallel client-driven work.
- **The caveat both sides must carry**: the Flight decoder bugs affected RSC apps with zero Server Functions. Moving mutations to route handlers does not remove that exposure in an App Router app. Only upgrading does.

### D. Vercel's influence on React

- **Critique** (summarized by devclass, 2025-06-23, not verified at the primary source): several core React team members work at Vercel, Next.js was the main full RSC implementation, and the App Router aligns with Vercel's infrastructure.
- **Rebuttal** (Mark Erikson, "The State of React and the Community in 2025", via the same summary): RSC was the React team's own vision, and Meta needed an outside sponsor to build it.
- **Structural fact** (https://react.dev/blog/2026/02/24/the-react-foundation): React, React Native, and JSX moved to the React Foundation under the Linux Foundation. Platinum members include Meta, Vercel, Microsoft, Amazon, Expo, Callstack, Software Mansion, and Huawei. "React's technical governance will always be independent from the React Foundation board." The technical governance structure is still being finalized. **VOLATILE.**

### E. Next.js caching: magic or explicit

- **Critique**, conceded by Vercel itself: the Next.js 13 and 14 defaults cached implicitly and confused developers. Next.js 15 reversed them, and 16.3 announced "no hidden or implicit caching".
- **The Next.js team's current position**: explicit `'use cache'` plus Suspense gives a static shell with dynamic holes, and build-time validation forces the choice up front.
- **Residual critique** (inferred from the advisory record): within a year the new model's cache keys and fill sharing produced two cross-request leaks (CVE-2026-103004, CVE-2026-94544) and an RCE (GHSA-673q-mjm6-r5xr), and two caching models now coexist in one major version.

### F. SPA or server-first

- **Server-first**: React sunset Create React App (2025-02-14) and recommends frameworks. Next.js defaults to server rendering.
- **SPA-capable**: React Router keeps an SPA mode ("Lowest Common Mode"), and TanStack Start lets each route choose its rendering. An internal tool behind login, with no SEO need, gains little from SSR and pays its complexity.

---

## 10. Anti-pattern catalog

Each entry: pattern, trigger, consequence, fix.

1. **Unauthenticated Server Function.** Exported `'use server'` `deleteNote(id)` with no session check. Any client POSTs the discoverable action ID with any ID and deletes another user's note. Authenticate and check ownership inside the function, or in the DAL it calls.
2. **Authentication without authorization.** The action checks `session.user` but not `note.ownerId === session.user.id`. A logged-in user mutates other users' records. Load the resource, compare owner or tenant, then mutate.
3. **Page guard assumed to cover inline actions.** The page redirects non-admins, and an inline action deletes records with no check. Re-verify in the action.
4. **Middleware / proxy as the only authorization.** A matcher guards `/admin`. Bypassed by headers, `.rsc` URLs, parameter injection, or locale handling. Authorize in the page, DAL, or action. Use the proxy for redirects and coarse gating only.
5. **Identity taken from arguments.** `updateProfile(userId, data)`. Any profile can be edited. Derive the user from the session.
6. **Unvalidated arguments.** `formData.get("qty") as number`. `NaN`, negatives, and objects reach the database. Parse with a schema at the top of every action.
7. **Raw rows across the boundary.** `<Profile user={row} />` or `return db.user.update(...)`. Hashes, emails, and flags are in the RSC payload. Return a DTO with named fields.
8. **Taint as the control.** Taint the user, then pass `{ ...user }`. The spread is untainted. Use DTOs. Taint is an experimental backstop.
9. **No `server-only` on the DAL.** A client component imports through a barrel. The database client or a secret ships, or the build fails late. `import "server-only"` at the top of the DAL and secret modules.
10. **`.server.ts` naming kept as the guard after moving React Router to RSC mode.** The convention is no longer enforced. Add `import "server-only"` or `denyImports`.
11. **Secret in a `'use client'` module or a `NEXT_PUBLIC_` variable.** Inlined into public JavaScript. Keep secrets in the DAL.
12. **Sensitive values captured in an inline action's closure.** The value round-trips through the client. Capture an ID and re-fetch on the server.
13. **User data in a shared `'use cache'` entry.** The key omits the user. The first user's data reaches every user. Pass the user ID as an argument, use `'use cache: private'`, or do not cache.
14. **Cached authorization decision.** Revoked access persists for the cache lifetime. Do not cache it, or invalidate on change.
15. **`revalidateTag` (stale-while-revalidate) after the user's own write.** The user sees stale data and resubmits. `updateTag`.
16. **No revalidation after a mutation.** Stale UI until expiry. `updateTag`, `revalidatePath`, or `refresh()` before returning.
17. **`redirect()` or `notFound()` inside `try` / `catch`.** The redirect is swallowed, or a 404 renders as 200. Move it after the `try`, or `unstable_rethrow` first in the `catch`.
18. **Revalidate after `redirect()`.** It never runs. Revalidate first.
19. **Server Function used for reads.** Reads serialize and queue behind mutations, and POST is not cacheable. Fetch in a Server Component, or use a GET route handler for client-driven reads.
20. **Sequential awaits in nested Server Components.** Time to first byte is the sum of latencies. Start promises in parallel, preload, push awaits behind Suspense.
21. **`cache()` defined inside a component.** No deduplication. Define at module scope and pass primitives.
22. **`React.cache` expected inside `'use cache'`.** The cached child sees nothing. Pass data as arguments.
23. **Request data awaited inside `'use cache'`.** Build hangs 50 seconds then fails, or the error appears only under `next start`. Read runtime values outside and pass plain values.
24. **Time, randomness, or environment read in a prerendered route.** The value freezes at build. `await connection()` inside Suspense, or cache deliberately.
25. **Environment-dependent render output.** `typeof window` branch, locale formatting, invalid nesting. Mismatch, client re-render, or handlers on the wrong elements. Stable server values, `useId`, explicit locale and time zone, valid HTML, `use(browser())`.
26. **`suppressHydrationWarning` on a container.** No effect on children. Put it on the exact element, or fix the source.
27. **`isClient` effect over a large subtree.** Double render, layout shift, content missing from SSR. Narrow it, or use `use(browser())` on 19.3+, or CSS.
28. **UI that branches on `error.message` from a server throw.** Generic in production. Return expected errors as values. Log the digest.
29. **`e.message` returned from an action.** Internal error text reaches the client. Return curated messages and log the raw error.
30. **Hand-written error boundary around routes.** Catches redirects. Use `catchError` or `error.tsx`.
31. **Broad `allowedOrigins`.** `["**.com"]` or `"null"`. CSRF protection weakened. List exact hosts and forward `X-Forwarded-Host`.
32. **Unpinned Host on a custom server.** SSRF through Server Actions. Pin and validate Host at the edge, and upgrade.
33. **CDN that does not vary on RSC headers.** RSC payloads served for HTML URLs. Vary on the RSC headers or bypass RSC requests.
34. **RSC framework below the security floor.** Unauthenticated RCE. Upgrade, and check the vendored copy through the `next` version.
35. **Non-idempotent action with offline retry.** Duplicate or late charges. Idempotency keys.
36. **File-level `'use cache'` module imported by a client component.** Its exports are unauthenticated endpoints. Keep it server-imported, or add authorization inside.
37. **Draft Mode with shared cache fills on 16.3.0 to 16.3.x.** Unpublished content in public prerenders. Upgrade, and pass the draft flag as an argument.
38. **Nested `'use cache'` reading root params before 16.3.8.** Content crosses locales. Upgrade.
39. **`'use client'` on every file.** Every file is a potential client entry, and the real boundary is invisible. Mark entry points only.
40. **Server Component imported by a Client Component to "reuse" it.** It becomes a client component, with its data access, in the client bundle. Pass it as `children` instead.

---

## 11. Authorities

| Source | Good for |
|---|---|
| https://react.dev/reference/rsc/use-client , `/use-server` , `/server-functions` , `/server-components` | Normative serialization lists, directive semantics, React's own Server Function security statements |
| react.dev/blog | Release posts and security posts (2025-12-03, 2025-12-11 with appended updates) |
| GitHub advisories for `facebook/react` and `vercel/next.js` | Exact affected and fixed ranges. Faster and more precise than blogs |
| https://nextjs.org/docs/app/guides/data-security | DAL and DTO pattern, the Server Action security model, the audit checklist |
| Next.js caching and `use-cache` references, and the caching-without-cache-components guide | Both caching models, key composition, storage per environment |
| Next.js release posts (15, 16, 16.1 to 16.4) and Markbåge's "Our Journey with Caching" | Version boundaries and the team's rationale |
| Dan Abramov, overreacted.io | The conceptual model (two module graphs, directives as typed script and typed fetch, poison pills). Not a security reference |
| reactrouter.com versioned RSC docs | React Router RSC APIs and the `.server` module caveat |
| tanstack.com and remix.run blogs | The opt-in and departure positions |
| OWASP IDOR cheat sheet (linked from the Next.js guide) | Why authorization belongs inside the action |

---

## 12. Severity rubric

- **Blocker**: an exported Server Function, route handler, or file-level `'use cache'` module that mutates or reads another user's data with no authorization inside it. Middleware / proxy as the only authorization for a sensitive route. A shared cache entry that serves user-specific data to other users. A secret in a client module or `NEXT_PUBLIC_` variable. An RSC framework version below the current security floor (confirm the floor first).
- **Major**: authentication without an ownership check where the resource is low-sensitivity. Raw database rows passed as props or returned from actions. Identity accepted as an argument. Unvalidated action arguments reaching the database. `redirect()` or `notFound()` swallowed by `try` / `catch`. Missing revalidation after a mutation. `revalidateTag` where read-your-writes is required. A broad `allowedOrigins`. A cached authorization decision.
- **Minor**: a request waterfall in a non-critical route. `cache()` defined in a component. `suppressHydrationWarning` on a container. An `isClient` subtree that could be narrower. `error.message` shown in UI. `'use client'` on non-entry files.
- **Nit**: "Server Actions" versus "Server Functions" naming. A `middleware.ts` file on a Node runtime project that could be `proxy.ts`.
- **Insight**: a structural option: introduce a DAL, move a read from a Server Function to a Server Component, pass a server subtree as `children`, adopt `use(browser())`, run `next experimental-analyze` to audit the client graph.

Every finding names who can reach what. Not "add auth to this action", but "`actions/notes.ts:12` exports `deleteNote(id)` with no session or ownership check. The action ID is in the public client chunk, so any visitor can POST it with another user's note ID and delete that note. Call `requireUser()` and compare `note.ownerId` before the delete."

---

Source research: `~/.claude/local/research-notes/react-server-research.md`

## Changelog

- 2026-10-14: Created. Researched against React 19.3.1, Next.js 16.4.1, React Router 8.4.0, TanStack Start 1.0. Two critical advisories (GHSA-xqc2-xf4c-x3h3, GHSA-673q-mjm6-r5xr) published the same day, with no CVE assigned yet.
