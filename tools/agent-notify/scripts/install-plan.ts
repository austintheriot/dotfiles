export const LABEL = "dev.dotfiles.agent-notifyd";
export const BUNDLE_ID = "dev.dotfiles.agent-notifyd";
export const EXECUTABLE = "agent-notifyd";

export type InstallInputs = {
  home: string;
  uid: number;
  tmux: string | null;
  aerospace: string | null;
  brewBin: string;
  stamp: string;
};

export type InstallPlan = {
  appDir: string;
  executablePath: string;
  infoPlist: string;
  launchAgentPath: string;
  launchAgent: string;
  stateDir: string;
  stampPath: string;
  logDir: string;
  domain: string;
  targetDir: string;
};

export function xmlEscape(text: string): string {
  return text.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;").replaceAll("'", "&apos;");
}

const HEADER = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>`;
const FOOTER = `</dict>
</plist>
`;

const entry = (key: string, value: string, indent = "  "): string =>
  `${indent}<key>${xmlEscape(key)}</key>\n${indent}<string>${xmlEscape(value)}</string>`;

export function renderInfoPlist(): string {
  return [
    HEADER,
    entry("CFBundleIdentifier", BUNDLE_ID),
    entry("CFBundleExecutable", EXECUTABLE),
    entry("CFBundleName", EXECUTABLE),
    entry("CFBundlePackageType", "APPL"),
    entry("LSMinimumSystemVersion", "26.0"),
    "  <key>LSUIElement</key>\n  <true/>",
    FOOTER,
  ].join("\n");
}

export function renderLaunchAgent(inputs: InstallInputs, executablePath: string, logDir: string): string {
  const environment: Array<[string, string]> = [
    ["PATH", `${inputs.brewBin}:/usr/bin:/bin:/usr/sbin:/sbin`],
    ["LANG", "en_US.UTF-8"],
    ["RUST_BACKTRACE", "1"],
  ];
  if (inputs.tmux !== null) environment.push(["AGENT_NOTIFY_TMUX", inputs.tmux]);
  if (inputs.aerospace !== null) environment.push(["AGENT_NOTIFY_AEROSPACE", inputs.aerospace]);
  return [
    HEADER,
    entry("Label", LABEL),
    `  <key>ProgramArguments</key>\n  <array>\n    <string>${xmlEscape(executablePath)}</string>\n  </array>`,
    "  <key>RunAtLoad</key>\n  <true/>",
    "  <key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>",
    entry("LimitLoadToSessionType", "Aqua"),
    entry("ProcessType", "Interactive"),
    entry("StandardErrorPath", `${logDir}/launchd-stderr.log`),
    `  <key>EnvironmentVariables</key>\n  <dict>\n${environment.map(([key, value]) => entry(key, value, "    ")).join("\n")}\n  </dict>`,
    FOOTER,
  ].join("\n");
}

export function planInstall(inputs: InstallInputs): InstallPlan {
  const appDir = `${inputs.home}/Applications/agent-notifyd.app`;
  const executablePath = `${appDir}/Contents/MacOS/${EXECUTABLE}`;
  const stateDir = `${inputs.home}/Library/Application Support/agent-notify`;
  const logDir = `${inputs.home}/Library/Logs/agent-notify`;
  return {
    appDir,
    executablePath,
    infoPlist: renderInfoPlist(),
    launchAgentPath: `${inputs.home}/Library/LaunchAgents/${LABEL}.plist`,
    launchAgent: renderLaunchAgent(inputs, executablePath, logDir),
    stateDir,
    stampPath: `${stateDir}/installed-stamp`,
    logDir,
    domain: `gui/${inputs.uid}`,
    targetDir: `${inputs.home}/.cache/agent-notify/target`,
  };
}

export const STAMP_PATHS: ReadonlyArray<string> = ["tools/agent-notify", "crates/agent-notify-core", "crates/Cargo.toml", "crates/Cargo.lock"];

export type BuildPlan = { command: string[]; env: Record<string, string | undefined>; builtPath: string };

export function planBuild(plan: InstallPlan, stamp: string, environment: Record<string, string | undefined>): BuildPlan {
  return {
    command: ["cargo", "build", "--release", "--locked", "-p", EXECUTABLE, "--target-dir", plan.targetDir],
    env: { ...environment, CARGO_TARGET_DIR: plan.targetDir, CONFIG_MANIFEST_STAMP: stamp },
    builtPath: `${plan.targetDir}/release/${EXECUTABLE}`,
  };
}

export type ReloadStep = { step: "kickstart" } | { step: "bootout" } | { step: "bootstrap"; tries: number; delayMs: number };

export const BOOTSTRAP_TRIES = 5;
export const BOOTSTRAP_RETRY_DELAY_MS = 500;

export function planReload(state: { loaded: boolean; plistChanged: boolean }): ReadonlyArray<ReloadStep> {
  if (state.loaded && !state.plistChanged) return [{ step: "kickstart" }];
  return [{ step: "bootout" }, { step: "bootstrap", tries: BOOTSTRAP_TRIES, delayMs: BOOTSTRAP_RETRY_DELAY_MS }];
}

export function stampFrom(files: ReadonlyArray<{ path: string; bytes: Uint8Array }>): string {
  const hasher = new Bun.CryptoHasher("sha256");
  for (const file of [...files].sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0))) {
    hasher.update(file.path);
    hasher.update("\0");
    hasher.update(String(file.bytes.length));
    hasher.update("\0");
    hasher.update(file.bytes);
  }
  return hasher.digest("hex").slice(0, 16);
}
