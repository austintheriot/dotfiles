import { mkdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { replaceVerified, writeAtomic } from "./install-files";
import { LABEL, planBuild, planInstall, planReload, STAMP_PATHS, stampFrom, type InstallPlan, type ReloadStep } from "./install-plan";

const toolRoot = join(import.meta.dir, "..");
const home = process.env.HOME ?? "";
const skipLaunchd = process.argv.includes("--no-launchd");

if (process.platform !== "darwin") {
  console.error("thumper install: skipped, macOS only in this version (plan 3 adds Linux)");
  process.exit(0);
}
if (home === "") {
  console.error("thumper install: HOME is not set");
  process.exit(1);
}

type RunOptions = { cwd?: string; env?: Record<string, string | undefined>; quiet?: boolean };

function attempt(command: string[], options: RunOptions = {}): { success: boolean; exitCode: number; stdout: string } {
  const result = Bun.spawnSync(command, { cwd: options.cwd ?? toolRoot, env: options.env ?? process.env, stdout: "pipe", stderr: options.quiet === true ? "ignore" : "inherit" });
  return { success: result.success, exitCode: result.exitCode, stdout: result.stdout.toString() };
}

function run(command: string[], options: RunOptions = {}): string {
  const result = attempt(command, options);
  if (!result.success) {
    throw new Error(`${command.join(" ")} failed with exit ${result.exitCode}`);
  }
  return result.stdout;
}

async function sourceStamp(): Promise<string> {
  const listed = run(["git", `--git-dir=${home}/.cfg`, `--work-tree=${home}`, "ls-files", "--", ...STAMP_PATHS], { cwd: home });
  const paths = listed.split("\n").filter((path) => path !== "");
  const files = await Promise.all(paths.map(async (path) => ({ path, bytes: new Uint8Array(await readFile(join(home, path))) })));
  return stampFrom(files);
}

function stagedLaunchAgentPath(plan: InstallPlan): string {
  return join(plan.stateDir, "launch-agent.plist.staged");
}

async function readIfPresent(path: string): Promise<string | null> {
  try {
    return await readFile(path, "utf8");
  } catch {
    return null;
  }
}

async function install(plan: InstallPlan, stamp: string): Promise<void> {
  run(["bun", "install", "--frozen-lockfile"], { cwd: join(toolRoot, "ui") });
  run(["bun", "run", "build"], { cwd: join(toolRoot, "ui") });
  run(["cargo", "deny", "check"]);
  const build = planBuild(plan, stamp, process.env);
  run(build.command, { env: build.env });
  await replaceVerified({ source: build.builtPath, destination: plan.executablePath, expectedStamp: stamp });
  await writeAtomic(join(plan.appDir, "Contents", "Info.plist"), plan.infoPlist);
  run(["/usr/bin/codesign", "--force", "--sign", "-", plan.appDir]);
  await mkdir(plan.stateDir, { recursive: true, mode: 0o700 });
  await mkdir(plan.logDir, { recursive: true, mode: 0o700 });
  await writeAtomic(skipLaunchd ? stagedLaunchAgentPath(plan) : plan.launchAgentPath, plan.launchAgent);
  await writeAtomic(plan.stampPath, `${stamp}\n`);
}

function bootstrapWithRetry(plan: InstallPlan, tries: number, delayMs: number): void {
  const command = ["/bin/launchctl", "bootstrap", plan.domain, plan.launchAgentPath];
  for (let i = 1; i <= tries; i++) {
    const result = attempt(command);
    if (result.success) return;
    if (i < tries) Bun.sleepSync(delayMs);
    else
      throw new Error(
        `${command.join(" ")} failed ${tries} times, last exit ${result.exitCode}; ${LABEL} is now unloaded until the next login or until you run: /bin/launchctl bootstrap ${plan.domain} ${plan.launchAgentPath}`,
      );
  }
}

function applyReloadStep(plan: InstallPlan, step: ReloadStep): void {
  const target = `${plan.domain}/${LABEL}`;
  switch (step.step) {
    case "kickstart":
      run(["/bin/launchctl", "kickstart", "-k", target]);
      return;
    case "bootout":
      attempt(["/bin/launchctl", "bootout", target]);
      return;
    case "bootstrap":
      bootstrapWithRetry(plan, step.tries, step.delayMs);
      return;
  }
}

function reload(plan: InstallPlan, previousLaunchAgent: string | null): void {
  const loaded = attempt(["/bin/launchctl", "print", `${plan.domain}/${LABEL}`], { quiet: true }).success;
  const plistChanged = previousLaunchAgent !== plan.launchAgent;
  for (const step of planReload({ loaded, plistChanged })) applyReloadStep(plan, step);
}

async function main(): Promise<void> {
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
  const previousLaunchAgent = skipLaunchd ? null : await readIfPresent(plan.launchAgentPath);
  await install(plan, stamp);
  if (skipLaunchd) {
    console.log(`thumper install: installed ${plan.appDir} (stamp ${stamp}); launchd not touched`);
    console.log(`thumper install: staged launch agent at ${stagedLaunchAgentPath(plan)}`);
    console.log(`thumper install: it would install to ${plan.launchAgentPath}`);
  } else {
    reload(plan, previousLaunchAgent);
    console.log(`thumper install: installed and restarted ${LABEL} (stamp ${stamp})`);
  }
}

try {
  await main();
} catch (error) {
  console.error(`thumper install: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
