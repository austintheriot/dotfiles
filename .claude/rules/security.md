# Security Review Principles

A reference for evaluating code from a security-engineering lens. Used by the `security` subagent. Distinct from `bug-patterns.md`'s "Security-shaped bugs" section -- that catches *patterns* (injection, timing channels, secret logging). This file is the *design and mental-model* layer: AuthN/AuthZ design, threat-modeling, supply-chain integrity, OWASP Top 10 / CWE Top 25, secrets handling, dependency hygiene.

The core thesis: most security incidents are not novel vulnerabilities; they are missing controls or correct controls applied in the wrong place. The reviewer's lens is "what is the trust boundary here, what crosses it, who can manipulate the input, and where does the control live?"

---

## Threat-model framing

Before pattern-matching, ask the right four questions for any non-trivial change:

1. **What are the assets?** Data (user PII, payment data, secrets, intellectual property), capability (admin endpoints, privileged operations), availability (the service itself).
2. **Who are the adversaries?** External (anonymous, authenticated-attacker, compromised-account), internal (malicious insider, supply-chain compromise, compromised CI), accidental (developer mistake, dependency bug). Different adversaries demand different controls.
3. **What are the trust boundaries?** Where does untrusted input enter the system? Where does authenticated-but-not-authorized input cross into authorized work? Where does internal-trusted state get serialized for external consumption? Every boundary is a control point.
4. **What is the control?** Validation, AuthN, AuthZ, encryption, rate limiting, logging, audit. Each control should be named, owned, and tested.

**Flag**: a change that crosses a trust boundary without naming the boundary or its control. "Accepts a user-supplied URL and fetches it" is a question, not a feature -- where's the SSRF defense?

---

## OWASP Top 10 (2021) and what each looks like in code

The categories most often raised in real reviews. Pattern-match against the design, not just the syntax.

### A01 Broken Access Control (most common in real apps)

The single most-cited category in OWASP's top 10. AuthZ -- "this authenticated user is allowed to do *this specific thing to this specific resource*" -- is repeatedly broken at the application layer.

**Common shapes**:
- *Insecure Direct Object Reference (IDOR)*: `/api/users/:id/orders` where the handler trusts `:id` from the URL without checking it matches `req.user.id`. Horizontal privilege escalation -- user 456 reads user 123's orders.
- *Missing object-level authorization*: the handler verifies "is logged in" but not "owns this resource."
- *Vertical privilege escalation*: regular user hits an admin endpoint that only the UI hides.
- *Confused deputy*: a service calls a privileged backend on behalf of an unauthenticated client whose ID it forwards trustingly.
- *AuthZ-by-frontend*: the UI hides admin buttons; the API does not enforce.
- *Method-tampering*: handler allows GET / POST / PUT / DELETE on a resource where only one should be permitted.
- *CORS misconfiguration*: `Access-Control-Allow-Origin: *` with credentials, or reflecting `Origin` without an allowlist.
- *Path-relative redirects*: `redirect(req.query.next)` without validating the destination.

**Defenses**: every endpoint's first non-auth step is an explicit authorization check that compares `req.user` to the resource. Centralize via a policy / casbin / OPA layer where possible. Tests assert "user A cannot access user B's resource" as a positive test, not just "user A can access user A's."

### A02 Cryptographic Failures (was "Sensitive Data Exposure")

**Common shapes**:
- *Plaintext at rest*: passwords stored without hashing, PII in plain DB columns when the data classification requires encryption.
- *Wrong primitive*: MD5 / SHA-1 for password hashing (use bcrypt / scrypt / argon2 / pbkdf2); ECB mode AES; static IVs; missing AEAD.
- *Custom crypto*: hand-rolled ciphers, hand-rolled signature schemes, "I'll just XOR the bytes."
- *Hardcoded keys / IVs / salts*: especially in source. Even a "test key" in dev becomes a prod key.
- *Weak randomness for security purposes*: `Math.random()` / `rand()` for tokens; needs CSPRNG (`crypto.randomBytes`, `getrandom`, `SecureRandom`).
- *TLS verification disabled*: `rejectUnauthorized: false`, `verify=False`, `--insecure`. Disabling cert verification in production is a critical defect.
- *Mixed-content / downgrade*: serving cookies without `Secure` and `HttpOnly`; allowing protocol downgrade.

**Defenses**: standard libraries (BoringSSL, libsodium, ring); password hashing with cost factor tunable; PII encryption with KMS-managed keys; certificate verification always on; secret rotation cadence documented.

### A03 Injection

Covered partly in `bug-patterns.md` § Encoding/escaping; the *design* version:

- *SQL injection*: parameterized queries everywhere; ORM that doesn't bypass via raw query strings; no string concatenation into queries.
- *NoSQL injection*: MongoDB query operators (`$ne`, `$gt`) accepted from user input; trust-boundary failure.
- *Command injection*: `exec(cmd + userInput)` -- always use `execFile` / `spawn` with array args; never shell-form.
- *LDAP / XPath / OGNL injection*: same shape, different language.
- *Server-Side Template Injection (SSTI)*: rendering user input as a template (`render({{user.input}})`); allows RCE in many engines (Jinja2, FreeMarker, Velocity).
- *Header injection / response splitting*: user input in HTTP headers without CRLF stripping.
- *Log injection*: user-controlled newlines / forged log lines; especially in logs piped to search systems where attackers can spoof correlation IDs.

### A04 Insecure Design (added 2021)

This is the *missing-control* category. Not a bug pattern; an absent feature.

**Common shapes**:
- *Missing rate limiting* on auth endpoints, password reset, OTP verification, costly read endpoints.
- *Missing brute-force / credential-stuffing defense*: lockout, CAPTCHA, or proof-of-work.
- *Trust in client-supplied state*: discount codes calculated on the client; prices accepted from the cart; inventory derived from form fields.
- *No business-logic invariants*: "transfer requires balance >= amount" implemented only at the UI; backend trusts.
- *Privacy-violating defaults*: opt-out instead of opt-in for data collection.

**Defenses**: write down the abuse cases for every new feature. "What's the worst thing a motivated attacker could do with this endpoint in 1 hour?" If you cannot answer, the design is incomplete.

### A05 Security Misconfiguration

**Common shapes**:
- *Default credentials* still active in production.
- *Verbose error messages* leaking internal state, stack traces, ORM details, file paths.
- *Open admin / debug endpoints*: `/api/debug`, `/admin`, `/swagger-ui`, `/.git/`, `/actuator/heapdump` reachable in prod.
- *Permissive CORS / Content Security Policy (CSP) / cookie flags*.
- *Cloud misconfigurations*: S3 bucket public, security group `0.0.0.0/0:22`, IAM `*:*` policies.
- *Out-of-date dependencies* with known CVEs.

### A06 Vulnerable and Outdated Components

The supply chain. Reviewer's lens: every dependency is code you ship.

**Common shapes**:
- *Pinning by tag instead of by lockfile / SHA*: `package: latest`, `docker run ubuntu:latest`, `actions/checkout@main`.
- *Postinstall scripts* in package managers that execute arbitrary code at install time. Whitelist allowed packages with postinstall.
- *Typosquatting*: `reqeusts` instead of `requests`, `colors-js` instead of `colors`, package names confused for popular ones.
- *Dependency confusion*: internal package name registered on public registry takes priority.
- *Unsigned artifacts*: containers, packages, releases without provenance.
- *Tooling on the critical path with no audit*: a single Maven plugin, a single npm postinstall script.
- *No `npm audit` / `cargo audit` / `pip-audit` / `bundle audit`* in CI.
- *Old Node / Python / Ruby* with known CVEs.

**Defenses**: SBOM (software bill of materials); lockfile committed; CI runs vulnerability scan; security-only updates auto-merged (Dependabot, Renovate); SLSA-style provenance for built artifacts.

### A07 Identification and Authentication Failures

**Common shapes**:
- *Session fixation*: not rotating session ID on login.
- *No session expiry / no idle timeout / no concurrent-session limit*.
- *JWT pitfalls*: `alg=none` accepted; HS256 with public key used as secret; no `exp` claim; signing key in source; no key rotation.
- *Password policies missing or wrong*: short minimum, no breach-list check (HIBP), no rate limit on attempts.
- *MFA bypass paths*: backup codes never expire; SMS as the only second factor; password reset bypasses MFA.
- *OAuth misconfigurations*: implicit flow when authorization-code-with-PKCE is required; missing `state` parameter (CSRF on OAuth); wildcard redirect_uri.
- *Credential exposure in URLs* (GET parameters logged in proxies, browser history, referrer headers).

### A08 Software and Data Integrity Failures

**Common shapes**:
- *Insecure deserialization*: Java `ObjectInputStream`, Python `pickle.loads`, Ruby YAML.load, .NET `BinaryFormatter` on untrusted data. Direct RCE in many cases.
- *Auto-update / CI without integrity checks*: app downloads update over HTTP, unsigned; CI fetches scripts via `curl | sh`.
- *Cache poisoning*: shared CDN or HTTP cache fooled by header manipulation.

### A09 Security Logging and Monitoring Failures

The audit-trail category. Notable absent things, not present bugs.

**Common shapes**:
- *No audit log for privileged actions*: who created an admin user, who exfiltrated data, who reset a password.
- *No alerts on auth anomalies*: 1000 failed logins from one IP, geo-impossible logins, sudden privilege escalation.
- *Logs missing actor / target / outcome*: "user did action" without user ID, resource ID, success/fail.
- *Logs containing secrets*: tokens, passwords, full JWTs, raw card numbers (any of these is a finding).

### A10 Server-Side Request Forgery (SSRF)

The "the server fetches a URL the client controls" category.

**Common shapes**:
- *Image-by-URL features*: avatar uploaders, link previews, webhook URLs, OAuth `redirect_uri`.
- *URL parsing differs across stages*: validator parses one way, fetcher parses another (host confusion, URL parser confusion, IPv4-decimal/octal/hex tricks).
- *Bypasses of "no internal IPs"*: DNS rebinding (resolve external -> internal between check and fetch), redirect to internal address, alternate IP encodings.
- *Cloud metadata endpoints*: 169.254.169.254 (AWS / GCP / Azure), 100.100.100.200 (Alibaba). Fetched via SSRF, returns instance credentials.

**Defenses**: HTTP client that resolves DNS and rejects private IPs *before* connecting; allowlist of permitted hosts when possible; metadata-endpoint blocking at the egress layer; separate egress identity (no IAM role) for outbound user-driven fetches.

---

## CWE Top 25 categories not covered by OWASP Top 10

Worth scanning for in addition. The high-yield ones:

- **CWE-787 Out-of-bounds Write** -- in native code; the perennial #1 for memory-unsafe languages.
- **CWE-79 Cross-site Scripting (XSS)** -- output encoding at the rendering boundary, not input filtering. Reflected, stored, DOM-based variants.
- **CWE-89 SQL Injection** -- A03 above.
- **CWE-20 Improper Input Validation** -- "trust nothing from outside the boundary."
- **CWE-125 Out-of-bounds Read** -- often paired with 787.
- **CWE-78 OS Command Injection** -- A03 above.
- **CWE-416 Use After Free** -- memory-unsafe languages.
- **CWE-22 Path Traversal** -- `../../../etc/passwd` via untrusted filename.
- **CWE-352 CSRF** -- state-changing operations via GET, or POST without anti-CSRF token. SameSite cookies are the modern primary defense.
- **CWE-434 Unrestricted File Upload** -- file extension, content type, magic-byte mismatch, executable types reachable from the web root.
- **CWE-862 Missing Authorization** -- A01 above.
- **CWE-476 NULL Pointer Dereference** -- crash, denial-of-service. In memory-unsafe languages can be exploitable.
- **CWE-287 Improper Authentication** -- A07 above.
- **CWE-190 Integer Overflow** -- in size calculations especially; allocation sized wrong, then overflow.
- **CWE-502 Deserialization of Untrusted Data** -- A08 above.
- **CWE-77 Command Injection** -- A03 above.
- **CWE-119 Improper Restriction of Operations within Bounds of a Memory Buffer** -- memory-unsafe languages.
- **CWE-798 Use of Hard-coded Credentials** -- A05/A07 above.
- **CWE-918 SSRF** -- A10 above.
- **CWE-306 Missing Authentication for Critical Function** -- A01/A07 above.

---

## Secrets handling

A high-leverage subcategory. Almost every codebase has at least one of:

- *Secrets in source*: API keys, DB passwords, signing keys committed. Even in test fixtures.
- *Secrets in environment variables that leak* via logs, error pages, child-process env, debug endpoints (`/env`).
- *Secrets in build artifacts*: Docker layers, sourcemaps, frontend bundles, CI artifacts.
- *Secrets fetched insecurely*: from a config service over HTTP, from a shared file with weak ACLs.
- *No rotation story*: secrets that have not rotated in 2 years.
- *Secrets passed via CLI*: argv visible to `ps`.

**Defenses**: secret manager (Vault, AWS Secrets Manager, GCP Secret Manager, Doppler) with short-lived dynamic credentials; CI uses OIDC trust to fetch secrets without long-lived tokens; pre-commit hook scans for known secret shapes (gitleaks, trufflehog); detect-secrets baseline.

---

## Supply chain hygiene

The new high-impact category. Distinct enough from "dependency vulnerabilities" to flag separately.

- *Lockfile committed and respected* (`package-lock.json`, `yarn.lock`, `Cargo.lock`, `Gemfile.lock`, `poetry.lock`, `go.sum`). Builds use frozen versions.
- *Dependency installer can run code*: postinstall scripts (npm), `setup.py` execution (pip), Gemfile auto-require, Maven plugins. Audit and ideally disable.
- *Dependencies pulled from one registry*: not falling back to public when private has the canonical name (dependency-confusion attack).
- *Build provenance*: SBOM, signed artifacts, SLSA-level commitments.
- *Pinning by SHA, not by tag*: especially for GitHub Actions (`uses: actions/checkout@v4` is acceptable for major lines; `@<sha>` is the bulletproof form).
- *Audit cadence*: scheduled `npm audit fix`, `cargo audit`, Dependabot/Renovate enabled, vulnerability alerts routed to a humans.

---

## Defense-in-depth principles

The mindset, not the pattern catalog:

- **Fail closed**, not open. When the auth check is uncertain, deny.
- **Validate at every trust boundary**, even if an earlier boundary already did. Boundaries can be added later; redundant validation is correct.
- **Principle of least privilege**: every credential, role, IAM policy, file permission, port. "What's the minimum this needs?"
- **Defense in depth**: assume each layer will eventually fail. WAF + AuthZ + parameterized queries + minimum-privilege DB user + monitoring.
- **Security as a property of the system, not a layer**: bolted-on perimeter security with a soft interior fails the moment the perimeter is breached.
- **Audit trails for irreversible / privileged actions**: who, what, when, outcome. Stored separately from the action's primary record.

---

## What is NOT a security finding

Signal-to-noise matters. Don't flag:

- **Style / linter / typechecker concerns** unless they encode a security invariant (e.g., a custom lint rule blocking `eval` is the security invariant).
- **Theoretical vulnerabilities with no reachable trigger**. A SQL function called only by ops with raw-DB access is a different threat model than one called by a public endpoint.
- **Code that's deliberately defense-in-depth verbose**. Repeated validation at multiple layers is a feature, not surplus.
- **Auth code that doesn't log internals**. Crypto code that doesn't print intermediate state. Those are correct, not "debug-hostile" (the `debuggability` agent's lens, not yours).
- **Generic "use HTTPS" / "validate inputs" advice**. Flag specific instances with specific triggers.

---

## Severity

The `security` subagent uses the `panel-contract.md` rubric, but the security-specific calibration:

- **blocker**: reachable authentication / authorization bypass, RCE via deserialization or injection, secret exposure in source / artifact / log, SSRF to cloud metadata, missing input validation on a trust boundary with a specific exploit path.
- **major**: AuthZ design gap with no current exploit path but obvious bypass; weak cryptography in a sensitive path; lockfile missing; logged sensitive data; missing rate limit on auth-flow endpoints.
- **minor**: hardening opportunities (CSP could be stricter, cookie flag missing, security header absent), brittle dependency-pinning, audit-log gaps.
- **nit**: style-level security hygiene (cleartext URLs in comments, "TODO: use HTTPS later").
- **insight**: structural concerns ("this service has no AuthZ layer; consider adopting a policy engine").

Confidence: high when the finding is concrete and the trigger path is named; medium when reasoned from the design without verifying every exploitation path.

---

## Process for the security agent

1. Read the project's security-relevant docs first: `SECURITY.md`, `CLAUDE.md` security sections, `docs/threat-model.md` if present, any compliance docs (SOC2, HIPAA, PCI).
2. Identify the trust boundaries in the changed / surveyed code. Where does external input enter? Where do internal-trusted operations execute?
3. Walk OWASP Top 10 + the CWE high-yield list against each boundary. Most reviews touch 2-4 categories meaningfully.
4. Check secrets handling: is anything sensitive in source, logs, errors, artifacts?
5. Check supply-chain hygiene: lockfile present, scanning configured, dependencies pinned.
6. State trigger paths concretely. "User-supplied URL is passed to `axios.get()` on line 42, no host allowlist, internal cloud metadata at 169.254.169.254 is reachable" beats "possible SSRF."
7. Stay read-only.
