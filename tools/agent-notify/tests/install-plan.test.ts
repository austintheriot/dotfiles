import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { LABEL, planBuild, planInstall, planReload, renderInfoPlist, STAMP_PATHS, stampFrom, xmlEscape, type InstallInputs } from "../scripts/install-plan";

const inputs: InstallInputs = {
  home: "/Users/dev",
  uid: 501,
  tmux: "/opt/homebrew/bin/tmux",
  aerospace: "/opt/homebrew/bin/aerospace",
  brewBin: "/opt/homebrew/bin",
  stamp: "abc123",
};

const cratesDirectory = join(import.meta.dir, "../../../crates");

test("paths sit under the home directory", () => {
  const plan = planInstall(inputs);
  expect(plan.executablePath).toBe("/Users/dev/Applications/agent-notifyd.app/Contents/MacOS/agent-notifyd");
  expect(plan.launchAgentPath).toBe(`/Users/dev/Library/LaunchAgents/${LABEL}.plist`);
  expect(plan.stampPath).toBe("/Users/dev/Library/Application Support/agent-notify/installed-stamp");
  expect(plan.domain).toBe("gui/501");
  expect(plan.targetDir).toBe("/Users/dev/.cache/agent-notify/target");
});

test("the build writes to and copies from the planned target directory, whatever CARGO_TARGET_DIR holds", () => {
  const plan = planInstall(inputs);
  const build = planBuild(plan, "abc123", { CARGO_TARGET_DIR: "/Users/dev/.cache/config-manifest/target", HOME: "/Users/dev" });
  const flag = build.command.indexOf("--target-dir");
  expect(flag).toBeGreaterThan(-1);
  expect(build.command[flag + 1]).toBe(plan.targetDir);
  expect(build.builtPath).toBe(`${plan.targetDir}/release/agent-notifyd`);
  expect(build.env.CARGO_TARGET_DIR).toBe(plan.targetDir);
  expect(build.env.CONFIG_MANIFEST_STAMP).toBe("abc123");
  expect(build.env.HOME).toBe("/Users/dev");
});

test("the build plan is the same with no CARGO_TARGET_DIR set", () => {
  const plan = planInstall(inputs);
  const foreign = planBuild(plan, "abc123", { CARGO_TARGET_DIR: "/tmp/elsewhere" });
  const unset = planBuild(plan, "abc123", {});
  expect(foreign.command).toEqual(unset.command);
  expect(foreign.builtPath).toBe(unset.builtPath);
});

test.each([
  { loaded: true, plistChanged: false, steps: [{ step: "kickstart" }] },
  { loaded: true, plistChanged: true, steps: [{ step: "bootout" }, { step: "bootstrap", tries: 5, delayMs: 500 }] },
  { loaded: false, plistChanged: false, steps: [{ step: "bootout" }, { step: "bootstrap", tries: 5, delayMs: 500 }] },
  { loaded: false, plistChanged: true, steps: [{ step: "bootout" }, { step: "bootstrap", tries: 5, delayMs: 500 }] },
])("reload with loaded=$loaded and plistChanged=$plistChanged", ({ loaded, plistChanged, steps }) => {
  expect(planReload({ loaded, plistChanged })).toEqual(steps);
});

test("the stamp covers the workspace files the core crate inherits from", () => {
  const manifest = readFileSync(join(cratesDirectory, "agent-notify-core/Cargo.toml"), "utf8");
  expect(manifest).toContain("workspace = true");
  expect(STAMP_PATHS).toContain("crates/Cargo.toml");
  expect(STAMP_PATHS).toContain("crates/Cargo.lock");
  expect(STAMP_PATHS).toContain("crates/agent-notify-core");
  expect(STAMP_PATHS).toContain("tools/agent-notify");
});

test("the launch agent restarts only after a failure", () => {
  const agent = planInstall(inputs).launchAgent;
  expect(agent).toContain("<key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>");
  expect(agent).toContain("<key>LimitLoadToSessionType</key>\n  <string>Aqua</string>");
  expect(agent).toContain("<key>ProcessType</key>\n  <string>Interactive</string>");
  expect(agent).toContain("<key>RunAtLoad</key>\n  <true/>");
});

test("the launch agent carries absolute tool paths and a PATH with Homebrew first", () => {
  const agent = planInstall(inputs).launchAgent;
  expect(agent).toContain("<key>AGENT_NOTIFY_TMUX</key>\n    <string>/opt/homebrew/bin/tmux</string>");
  expect(agent).toContain("<key>AGENT_NOTIFY_AEROSPACE</key>\n    <string>/opt/homebrew/bin/aerospace</string>");
  expect(agent).toContain("<string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>");
  expect(agent).toContain("<key>RUST_BACKTRACE</key>\n    <string>1</string>");
});

test("a missing tool leaves its variable out", () => {
  const agent = planInstall({ ...inputs, aerospace: null }).launchAgent;
  expect(agent).not.toContain("AGENT_NOTIFY_AEROSPACE");
  expect(agent).toContain("AGENT_NOTIFY_TMUX");
});

test("text that reaches XML is escaped", () => {
  expect(xmlEscape(`a&b<c>"d'`)).toBe("a&amp;b&lt;c&gt;&quot;d&apos;");
  const agent = planInstall({ ...inputs, home: "/Users/a&b" }).launchAgent;
  expect(agent).toContain("/Users/a&amp;b/Applications/agent-notifyd.app/Contents/MacOS/agent-notifyd");
  expect(agent).not.toContain("/Users/a&b/");
});

test("the app bundle is a background agent with the macOS floor", () => {
  const info = renderInfoPlist();
  expect(info).toContain("<key>LSUIElement</key>\n  <true/>");
  expect(info).toContain("<key>LSMinimumSystemVersion</key>\n  <string>26.0</string>");
  expect(info).toContain("<key>CFBundleExecutable</key>\n  <string>agent-notifyd</string>");
  expect(info).toContain("<key>CFBundleIdentifier</key>\n  <string>dev.dotfiles.agent-notifyd</string>");
});

test("the stamp depends on content and path, not on input order", () => {
  const one = { path: "a.rs", bytes: new TextEncoder().encode("one") };
  const two = { path: "b.rs", bytes: new TextEncoder().encode("two") };
  expect(stampFrom([one, two])).toBe(stampFrom([two, one]));
  expect(stampFrom([one, two])).not.toBe(stampFrom([{ ...one, path: "c.rs" }, two]));
  expect(stampFrom([one, two])).not.toBe(stampFrom([one, { ...two, bytes: new TextEncoder().encode("twO") }]));
  expect(stampFrom([one])).toMatch(/^[0-9a-f]{16}$/);
});

test("the label matches the label the doctor command checks", () => {
  const source = readFileSync(join(cratesDirectory, "agent-notify/src/doctor.rs"), "utf8");
  const match = source.match(/pub const LAUNCHD_LABEL: &str = "([^"]+)";/);
  expect(match?.[1]).toBe(LABEL);
});

test("the state and log directories match the directories the daemon reads", () => {
  const source = readFileSync(join(cratesDirectory, "agent-notify-core/src/paths.rs"), "utf8");
  const stateSuffix = source.match(/pub fn state_directory[\s\S]*?home\.join\("([^"]+)"\)/)?.[1];
  const logSuffix = source.match(/pub fn log_directory[\s\S]*?home\.join\("([^"]+)"\)/)?.[1];
  const plan = planInstall(inputs);
  expect(plan.stateDir).toBe(`${inputs.home}/${stateSuffix}`);
  expect(plan.logDir).toBe(`${inputs.home}/${logSuffix}`);
});
