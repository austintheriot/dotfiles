import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { LABEL, planInstall, stampFrom, type InstallPlan } from "./install-plan";

const toolRoot = join(import.meta.dir, "..");
const home = process.env.HOME ?? "";
const skipLaunchd = process.argv.includes("--no-launchd");

if (process.platform !== "darwin") {
  console.error("agent-notify install: skipped, macOS only in this version (plan 3 adds Linux)");
  process.exit(0);
}
if (home === "") {
  console.error("agent-notify install: HOME is not set");
  process.exit(1);
}

function run(command: string[], options: { cwd?: string; env?: Record<string, string>; allowFailure?: boolean } = {}): string {
  const result = Bun.spawnSync(command, { cwd: options.cwd ?? toolRoot, env: { ...process.env, ...options.env }, stdout: "pipe", stderr: "inherit" });
  if (!result.success && options.allowFailure !== true) {
    console.error(`agent-notify install: ${command.join(" ")} failed with exit ${result.exitCode}`);
    process.exit(1);
  }
  return result.stdout.toString();
}

async function writeAtomic(path: string, text: string): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.tmp-${process.pid}`;
  await writeFile(temporary, text);
  await rename(temporary, path);
}

async function sourceStamp(): Promise<string> {
  const listed = run(["git", `--git-dir=${home}/.cfg`, `--work-tree=${home}`, "ls-files", "--", "tools/agent-notify", "crates/agent-notify-core"], { cwd: home });
  const paths = listed.split("\n").filter((path) => path !== "");
  const files = await Promise.all(paths.map(async (path) => ({ path, bytes: new Uint8Array(await readFile(join(home, path))) })));
  return stampFrom(files);
}

function stagedLaunchAgentPath(plan: InstallPlan): string {
  return join(plan.stateDir, "launch-agent.plist.staged");
}

async function install(plan: InstallPlan, stamp: string): Promise<void> {
  run(["bun", "install", "--frozen-lockfile"], { cwd: join(toolRoot, "ui") });
  run(["bun", "run", "build"], { cwd: join(toolRoot, "ui") });
  run(["cargo", "deny", "check"]);
  run(["cargo", "build", "--release", "--locked", "-p", "agent-notifyd"], { env: { CONFIG_MANIFEST_STAMP: stamp } });
  const built = join(home, ".cache/agent-notify/target/release/agent-notifyd");
  const contents = join(plan.appDir, "Contents");
  await mkdir(join(contents, "MacOS"), { recursive: true });
  await writeAtomic(join(contents, "Info.plist"), plan.infoPlist);
  const staged = `${plan.executablePath}.tmp-${process.pid}`;
  await Bun.write(staged, Bun.file(built));
  await chmod(staged, 0o755);
  await rename(staged, plan.executablePath);
  run(["/usr/bin/codesign", "--force", "--sign", "-", plan.appDir]);
  const reported = run([plan.executablePath, "--stamp"]).trim();
  if (reported !== stamp) {
    console.error(`agent-notify install: the bundle reports stamp ${reported}, expected ${stamp}`);
    process.exit(1);
  }
  await mkdir(plan.stateDir, { recursive: true, mode: 0o700 });
  await mkdir(plan.logDir, { recursive: true, mode: 0o700 });
  await writeAtomic(skipLaunchd ? stagedLaunchAgentPath(plan) : plan.launchAgentPath, plan.launchAgent);
  await writeAtomic(plan.stampPath, `${stamp}\n`);
}

function reload(plan: InstallPlan): void {
  run(["/bin/launchctl", "bootout", `${plan.domain}/${LABEL}`], { allowFailure: true });
  run(["/bin/launchctl", "bootstrap", plan.domain, plan.launchAgentPath]);
  run(["/bin/launchctl", "kickstart", "-k", `${plan.domain}/${LABEL}`]);
}

const brewBin = Bun.which("brew") === null ? "/usr/local/bin" : dirname(Bun.which("brew") ?? "/usr/local/bin/brew");
const stamp = await sourceStamp();
const plan = planInstall({
  home,
  uid: process.getuid?.() ?? 0,
  tmux: Bun.which("tmux"),
  aerospace: Bun.which("aerospace"),
  brewBin,
  stamp,
});
await install(plan, stamp);
if (skipLaunchd) {
  console.log(`agent-notify install: installed ${plan.appDir} (stamp ${stamp}); launchd not touched`);
  console.log(`agent-notify install: staged launch agent at ${stagedLaunchAgentPath(plan)}`);
  console.log(`agent-notify install: it would install to ${plan.launchAgentPath}`);
} else {
  reload(plan);
  console.log(`agent-notify install: installed and restarted ${LABEL} (stamp ${stamp})`);
}
