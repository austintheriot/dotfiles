# Build the pr-qa-base image

Every `vm up` clones `pr-qa-base`. Build it once per Mac with `vm image build`.

1. **Download the macOS base image**:

   ```sh
   tart clone ghcr.io/cirruslabs/macos-tahoe-base:latest pr-qa-base
   tart set pr-qa-base --display 1680x1050pt
   ```

   The desktop app opens at 1440x900, so a smaller screen cuts off its window.

2. **Build the image**. Run this command in a terminal:

   ```sh
   vm image build --checkout <checkout>
   ```

   It reads the project profile (see "Project profile" in [../README.md](../README.md)) and needs every file under its `provision.files` on your Mac. It runs only the steps that are due, in this order. When `provision` runs, every later step runs again.

   - `provision` installs the toolchain in the image and copies the profile's provision files into it.
   - `grant` grants screen recording, microphone, and AppleEvents to `tart-guest-agent`. It also sets the Chrome policy and turns off the prompts it can. It needs System Integrity Protection (SIP) off in the guest.
   - `approve` shows the macOS prompts that no script can answer, in the VM window.
   - `warm` runs the profile's `warm.installs` on a copy of the checkout, to fill the package caches. It needs each variable in `warm.secretEnv` set in an interactive zsh, and it sends those values to the VM for this install only. Without `--checkout`, or without a `warm` section in the profile, `build` skips `warm`.
   - `signin` restores the stored Google session into the image's Chrome. When no valid session exists, it guides you through a sign-in and stores the result.

   Each step that changes the image quits Chrome cleanly before the image stops. Clones then do not show the "Restore pages?" bubble.

3. **Answer what a person must answer**:

   - **Screen-capture prompt.** `vm image approve` shows `"tart-guest-agent" is requesting to bypass the system private window picker`. Click Allow in the VM window. macOS lets the approval lapse 30 days after the prompt. `vm image build` runs `approve` again when its marker is older than 28 days, and `vm doctor` flags the same age. `vm up` prints a warning when the approval is older than 28 days. After you press Enter, `approve` asks whether you clicked Allow on every prompt, and it records the approval only when you answer `y` or `yes`.
   - **Local-network prompt.** This prompt did not appear in testing. `approve` lists it. Click Allow only if it appears.
   - **Google sign-in.** When `signin` has no valid session, it asks you to sign in with your own account in the VM's Chrome, then to press Enter. Every VM clones this Chrome profile, so web QA gets through a Google sign-in gate (OAuth2 Proxy, for example) without a password. The profile's `signin.tabs` and `signin.steps` add the project's own sign-in pages.

The Google session is sealed in `~/.config/vm/google-session.enc`. The key is the Keychain item `vm-qa-google-session-key`.

Then run `vm doctor`. It shows one row per image step (`provision`, `grant`, `approve`, `signin`) and one row for the Google session.

## Run one step

Each step is also a command: `vm image provision`, `vm image grant`, `vm image approve`, `vm image signin`, and `vm image warm <checkout>`. `vm image grant` needs SIP off in the guest.

## Maintenance

| When | Run |
|---|---|
| Toolchains are out of date | `vm image build`. `provision` runs again only when the Bun version changes |
| Prompts appear after a `tart-guest-agent` upgrade | `vm image grant` |
| `vm doctor` says the approval is older than 28 days | `vm image approve` |
| `vm up` prints `Google session in pr-qa-base expired` | `vm image signin` |
| A lockfile the profile names changed | `vm image warm <checkout>` |
| Softnet is missing or lost its setuid bit | `./install-softnet.sh` |
