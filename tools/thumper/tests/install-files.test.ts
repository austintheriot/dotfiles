import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { replaceVerified, writeAtomic } from "../scripts/install-files";

type World = { directory: string; destination: string; cleanup: () => Promise<void> };

async function makeWorld(): Promise<World> {
  const directory = await mkdtemp(join(tmpdir(), "thumper-install-"));
  const destination = join(directory, "bundle", "thumperd");
  await mkdir(join(directory, "bundle"));
  await writeFile(destination, "old binary");
  return { directory, destination, cleanup: () => rm(directory, { recursive: true, force: true }) };
}

async function fakeBinary(world: World, reportedStamp: string): Promise<string> {
  const path = join(world.directory, "built-thumperd");
  await writeFile(path, `#!/bin/sh\necho ${reportedStamp}\n`);
  await chmod(path, 0o755);
  return path;
}

async function leftovers(world: World): Promise<string[]> {
  return (await readdir(join(world.directory, "bundle"))).filter((name) => name.includes(".tmp-"));
}

test("a binary that reports the expected stamp replaces the destination", async () => {
  const world = await makeWorld();
  const source = await fakeBinary(world, "abc123");

  await replaceVerified({ source, destination: world.destination, expectedStamp: "abc123" });

  expect(await readFile(world.destination, "utf8")).toContain("echo abc123");
  expect((await stat(world.destination)).mode & 0o111).not.toBe(0);
  expect(await leftovers(world)).toEqual([]);
  await world.cleanup();
});

test("a binary that reports another stamp leaves the destination unchanged and removes its staged copy", async () => {
  const world = await makeWorld();
  const source = await fakeBinary(world, "stale99");

  await expect(replaceVerified({ source, destination: world.destination, expectedStamp: "abc123" })).rejects.toThrow("stale99");

  expect(await readFile(world.destination, "utf8")).toBe("old binary");
  expect(await leftovers(world)).toEqual([]);
  await world.cleanup();
});

test("a binary that prints no stamp fails with its exit status and stderr and leaves the destination unchanged", async () => {
  const world = await makeWorld();
  const source = join(world.directory, "built-thumperd");
  await writeFile(source, "#!/bin/sh\necho 'no stamp compiled in' >&2\n");
  await chmod(source, 0o755);

  const attempt = replaceVerified({ source, destination: world.destination, expectedStamp: "abc123" });

  await expect(attempt).rejects.toThrow("printed no stamp (exit 0, signal none): no stamp compiled in");
  expect(await readFile(world.destination, "utf8")).toBe("old binary");
  expect(await leftovers(world)).toEqual([]);
  await world.cleanup();
});

test("a missing build output fails with its path and leaves the destination unchanged", async () => {
  const world = await makeWorld();
  const source = join(world.directory, "no-such-binary");

  await expect(replaceVerified({ source, destination: world.destination, expectedStamp: "abc123" })).rejects.toThrow(source);

  expect(await readFile(world.destination, "utf8")).toBe("old binary");
  expect(await leftovers(world)).toEqual([]);
  await world.cleanup();
});

test("a write whose rename fails removes its staged file", async () => {
  const world = await makeWorld();
  const blocked = join(world.directory, "bundle", "Info.plist");
  await mkdir(join(blocked, "occupied"), { recursive: true });

  await expect(writeAtomic(blocked, "text")).rejects.toThrow();

  expect(await leftovers(world)).toEqual([]);
  await world.cleanup();
});

test("a write lands the full text at the path", async () => {
  const world = await makeWorld();
  const path = join(world.directory, "nested", "state", "installed-stamp");

  await writeAtomic(path, "abc123\n");

  expect(await readFile(path, "utf8")).toBe("abc123\n");
  await world.cleanup();
});
