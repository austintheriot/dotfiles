import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { LABEL, planInstall, renderInfoPlist, stampFrom, xmlEscape, type InstallInputs } from "../scripts/install-plan";

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
