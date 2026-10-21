import { chmod, copyFile, mkdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

export type ReplaceRequest = { source: string; destination: string; expectedStamp: string };

async function stageThenRename(destination: string, stage: (staged: string) => Promise<void>): Promise<void> {
  await mkdir(dirname(destination), { recursive: true });
  const staged = `${destination}.tmp-${process.pid}`;
  try {
    await stage(staged);
    await rename(staged, destination);
  } catch (error) {
    await rm(staged, { force: true });
    throw error;
  }
}

export async function writeAtomic(path: string, text: string): Promise<void> {
  await stageThenRename(path, (staged) => writeFile(staged, text));
}

function reportedStamp(executable: string): string {
  const result = Bun.spawnSync([executable, "--stamp"], { stdout: "pipe", stderr: "pipe" });
  if (!result.success) {
    throw new Error(`${executable} --stamp failed with exit ${result.exitCode}: ${result.stderr.toString().trim()}`);
  }
  const stamp = result.stdout.toString().trim();
  if (stamp === "") {
    const signal = result.signalCode ?? "none";
    throw new Error(`${executable} --stamp printed no stamp (exit ${result.exitCode}, signal ${signal}): ${result.stderr.toString().trim()}`);
  }
  return stamp;
}

export async function replaceVerified(request: ReplaceRequest): Promise<void> {
  await stageThenRename(request.destination, async (staged) => {
    try {
      await copyFile(request.source, staged);
    } catch (error) {
      throw new Error(`could not copy the build output ${request.source}`, { cause: error });
    }
    await chmod(staged, 0o755);
    const reported = reportedStamp(staged);
    if (reported !== request.expectedStamp) {
      throw new Error(`the built binary reports stamp ${reported}, expected ${request.expectedStamp}`);
    }
  });
}
