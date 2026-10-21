# vm

`vm` runs QA for one pull request in its own macOS virtual machine (VM). Each VM is a Tart clone of the `pr-qa-base` image. It holds a signed-in Chrome for web QA, a copy of the PR's checkout for desktop QA, and a screen recorder that edits each QA step into a short MP4 with the cursor, clicks, and keys marked.

The tool is project-neutral. A project profile, a JSON file that lives outside this repository, says which checkout paths to copy, which files the image needs, and how to fill its package caches. See [Project profile](#project-profile).

## Install

You need a Mac with Apple silicon, [Bun](https://bun.sh), and [Homebrew](https://brew.sh).

```sh
./install.sh
```

`install.sh` installs Tart, ffmpeg, and Softnet (it asks for your sudo password once), links `~/.local/bin/vm`, and runs `vm doctor`. Rerun it at any time: it skips what is already in place.

`vm doctor` names each missing piece and the command that fixes it. The last piece is the base image. To build it, follow [setup/README.md](setup/README.md).

## Quick start

```sh
vm up 1234 ~/code/project/pr-1234          # boot pr-qa-1234 with that checkout
vm exec 1234 'cd app && bun install'       # run a command in ~/wt inside the VM
vm record start 1234 1 'Open a new file'   # start recording QA step 1
vm record mark 1234 key Meta+n             # show a key press in the video
vm record stop 1234                        # edit the step into /tmp/pr-review-qa/pr-qa-1234-step-1.mp4
vm pull 1234 /tmp/shot.png                 # copy a VM file to /tmp/pr-review-qa/
vm down 1234                               # delete the VM
```

In the VM, `cdp.mjs <port> open '<url>'` opens a URL in the current tab and shows an `Opened` chip.

The editor already shows typing it sees through the CDP (Chrome DevTools Protocol) driver as a `Typed` chip, and it masks password fields. Use `vm record mark <pr> note <text>` only for input the editor cannot see.

For web QA, add `vm browser <pr>` to an MCP config as a stdio server. It is a Playwright MCP server attached to the VM's Chrome. Its clicks, typing, and key presses are real macOS input, so the cursor shows in recordings.

## Commands

| Command | What it does |
|---|---|
| `vm up <pr> <worktree> [--headless]` | Clone `pr-qa-base` as `pr-qa-<pr>`, boot it, and copy the tracked files under the profile's `worktree.paths` into `~/wt` |
| `vm exec <pr> <command-line...>` | Run one zsh command line in `~/wt` inside the VM. The exit code is the command's. When the host reader of its stdout closes (for example `vm exec ... \| head`), it stops the command and exits 141. When the guest agent is not ready yet, it retries once, unless stdin carries data or the command already printed output. When the command fails, `vm exec` checks the VM network and can print a diagnosis line on stderr |
| `vm pull <pr> <vm-path> [--to <dir>]` | Copy one VM file to `/tmp/pr-review-qa/pr-qa-<pr>-<name>`, or to `<dir>/<name>` with `--to` |
| `vm record start <pr> <step> <caption>` | Start a full-screen recording of one QA step. It returns after the first frame is captured. When the recorder refuses (for example, a step is still recording), it prints the reason and exits 65 |
| `vm record stop <pr>` | Stop it and edit it into `/tmp/pr-review-qa/pr-qa-<pr>-step-<step>.mp4`. After every file reaches the host, it deletes the guest movie files |
| `vm record rerender <pr> <step>` | Edit a stopped step again from its kept sources |
| `vm record mark <pr> key\|note <text>` | Show a key press or a described action in the video. The text is at most 40 characters |
| `vm record pause <pr> [reason]` | Stop capturing until `resume`. Optional: use it only for a step that shows a secret in plain text. The reason (default `human sign-in`, at most 40 characters) shows on a dark card in the video |
| `vm record resume <pr>` | Start capturing again after `pause`. It returns after the first frame is captured, so the next action is in the video |
| `vm browser <pr>` | Stdio Playwright MCP server attached to the VM's Chrome |
| `vm restart <pr> [--headless]` | Stop the VM and boot it again. `~/wt` and its builds stay, and the lease stays. Use it when the network breaks after the host sleeps (Softnet), and after any guest reboot. It runs the same guest setup as `vm up`, without the worktree copy. It applies the same ownership rule as `vm up`: it refuses a VM that another live session owns, or a running VM with no lease |
| `vm image build [--checkout <path>]` | Run only the image steps that are due: `provision`, `grant`, `approve`, `warm`, `signin`. Needs a terminal. Without `--checkout`, it skips `warm` |
| `vm image provision` | Install the toolchain and the profile's `provision.files` in `pr-qa-base` |
| `vm image grant` | Grant every macOS permission QA needs in `pr-qa-base`. Needs SIP off in the guest |
| `vm image approve` | Show the macOS prompts that no script can answer, in the VM window, and wait for you to click Allow. Then ask for confirmation. `vm up` warns when the approval is older than 28 days. Needs a terminal |
| `vm image signin` | Restore or capture the Google session in `pr-qa-base`'s Chrome, then store it sealed on the host. Needs a terminal for the guided sign-in |
| `vm image warm <checkout>` | Fill the package caches in `pr-qa-base` with the profile's `warm.installs`, run on a copy of the checkout |
| `vm session inject-electron <pr> [--port <port>] [--url <substring>]` | Write the stored Google cookies into an Electron app in the PR VM, so an OAuth2 Proxy page passes without a password |
| `vm down <pr>` | Stop and delete the VM |
| `vm list` (alias `vm ls`) | List the QA VMs and the session that owns each one |
| `vm doctor` | Check that this Mac can run QA VMs, and show which profile `vm` reads |

`vm up` warns when the Google session in the new VM is signed out or expired. When it does, run `vm image signin`.

`vm <command> --help` shows the arguments for each command.

## Exit codes

| Code | Meaning | What to do |
|---|---|---|
| 0 | Success | |
| 64 | Bad arguments | Read the usage line it prints |
| 66 | A file to read is missing (`pull`, `rerender`), the VM does not exist (`restart`), `pr-qa-base` is missing (image steps), a provision file is missing (`vm image provision`), a required warm lockfile is missing (`vm image warm`), or the stored Google session is missing or invalid (`vm session inject-electron`) | Check the path. For `restart`, run `vm up`. For the image steps, follow [setup/README.md](setup/README.md). For the session, run `vm image signin` |
| 69 | The VM did not boot, `restart` could not stop it, `up` or `restart` could not read the VM list or process table (so ownership is unproven and the VM is left alone), Bun is missing or too old, a guest step of `vm image provision` failed, `vm image approve` or `vm image signin` has no terminal, SIP is on in the guest (`vm image grant`), `vm image approve` was not confirmed, or the Keychain read failed (`vm image signin`) | Read the reason it prints, then run `vm doctor`. For a missing terminal, run the command yourself in a terminal |
| 73 | Another live session owns that PR's VM (`up`, `restart`), `restart` found the VM running with no lease, or `pr-qa-base` is running (every image step and `vm up`) | Wait for it, or ask its owner (`vm list`) |
| 75 | The host already runs two macOS VMs, the most macOS allows | Retry later. Never stop another session's VM |
| 78 | The profile is invalid (`up`, `restart`, every image step), or it has no `warm` section (`vm image warm`) | Read the reason it prints, then fix the profile. `vm doctor` shows the same reason |
| 141 | `exec`: the host reader of stdout closed (for example `vm exec ... \| head`); the guest command was stopped | Nothing; this is the shell's closed-pipe code |

## VM ownership

macOS runs at most two macOS VMs at a time, so an abandoned VM blocks everyone. `vm up` writes a lease to `~/.cache/pr-review/vm-leases/` that names the owner: the nearest `claude` session above the caller, or the process in `QA_VM_OWNER_PID`. Before it clones, `up` deletes VMs whose owner has exited and stopped VMs with no lease. It never deletes a running VM whose owner it cannot find.

## Environment

| Variable | Effect |
|---|---|
| `VM_PROFILE` | The project profile to read. Default: `~/.claude/local/vm/profile.json` |
| `QA_VM_OWNER_PID` | The process the VM serves, for callers that are not a Claude session (a scheduler sets it) |
| `PR_REVIEW_VM_WINDOW=0` | Same as `--headless` on `vm up` and `vm restart` |

## Project profile

`vm up`, `vm restart`, and the image steps read a project profile. Other commands never read it, so a bad profile cannot break a running QA session. `vm` reads the file that `VM_PROFILE` names, or `~/.claude/local/vm/profile.json`. Keep the profile outside this repository: it names private paths, hosts, and secret names.

When no profile exists, `vm` copies the whole checkout into `~/wt` and adds no project setup. When the profile is invalid, those commands exit 78 and name the bad field. An unknown key is an error, so a typo cannot fall back to a default without a message.

Every section is optional.

| Key | Default | What it does |
|---|---|---|
| `version` | required, `1` | The profile format |
| `worktree.paths` | `["."]` | Git pathspecs, relative to the checkout. `vm up` copies only the tracked files under them into `~/wt`. `vm image warm` copies the same paths into `~/warm` |
| `fonts.ui`, `fonts.uiBold`, `fonts.mono` | none | `.woff2` files in the checkout. The video editor draws its captions and chips with them. Without them, it uses the system fonts |
| `inputShims` | `[]` | CDP driver scripts in the checkout that take the `cdp.mjs` command line (`<port> <command> ...`). `vm up` keeps each driver as `cdp.real.mjs` and puts a shim in its place, so desktop clicks and keys become real macOS input and show in videos |
| `provision.files` | `[]` | `{host, guest}` pairs. `vm image provision` copies each host file (a leading `~` is your home) to `guest`, relative to the guest home, readable only by its owner |
| `warm.installs` | none | `{dir, command, lockfile, required}` entries. `vm image warm` runs `command` in `~/warm/<dir>`. The hash of each `lockfile` decides when warm is due again. A missing lockfile stops warm when `required` is `true` (the default) and skips the install when it is `false` |
| `warm.secretEnv` | `[]` | Environment variable names. `vm image warm` reads each one from an interactive zsh on the host and sends it to the guest on stdin for that one install only |
| `signin.tabs`, `signin.steps` | `[]` | Extra tabs that `vm image signin` opens after the Google tab, and the extra instructions it prints |
| `session.keepCookies` | `[]` | `{namePrefix, domain}` rules. The sealed session keeps the Google account cookies plus each cookie whose name starts with `namePrefix` on `domain` or one of its subdomains |
| `chrome.mediaOrigins` | `[]` | Origins that the Chrome policy lets use the microphone and camera without a prompt |

An example for a project with a web app under `app/`:

```json
{
  "version": 1,
  "worktree": {"paths": ["app", ".claude"]},
  "inputShims": [".claude/skills/drive-desktop/cdp.mjs"],
  "provision": {"files": [{"host": "~/.config/example/login.json", "guest": ".config/example/login.json"}]},
  "warm": {
    "installs": [{"dir": "app", "command": "bun install", "lockfile": "app/bun.lock"}],
    "secretEnv": ["REGISTRY_TOKEN"]
  },
  "signin": {"tabs": ["https://staging.example.com/app"], "steps": ["Click \"Sign in with Google\" in the staging tab and let it load."]},
  "session": {"keepCookies": [{"namePrefix": "_oauth2_proxy", "domain": "example.com"}]},
  "chrome": {"mediaOrigins": ["https://meet.google.com", "https://[*.]example.com"]}
}
```

## Development

```sh
bun install
bun test
bun run check
```

### How recording works

In the VM, `video/proxy.ts` sits in front of Playwright MCP. It logs each browser input with its screen position, and it sends clicks, typing, and key presses as real macOS input through `cliclick`. `vm up` replaces the VM copy of the desktop driver `cdp.mjs` with `video/cdp-shim.mjs`, which does the same for desktop input. `video/cli.ts` runs `screencapture`. macOS stops a running capture when another app starts a media capture (Chrome joining a Meet call), so `cli.ts` supervises it and starts a new segment each time.

On the host, `video/render.ts` joins the segments and holds the last frame across each gap, so the input marks stay aligned. It speeds up the pauses (each pause plays in 2 s or less), draws a ring at each click and a chip for each input, adds the step caption, and writes a 15 fps H.264 MP4.

### Layout

`src/` is the CLI. `src/args.ts` declares the commands with Commander, and `src/profile.ts` parses the project profile. The pure modules (`args.ts`, `profile.ts`, `lease.ts`, `doctor-core.ts`) have tests beside them. `video/` is the recorder, the input proxy, and the video editor. `vm up` copies the files listed in `src/config.ts` into the VM, where they run on the image's Bun.
