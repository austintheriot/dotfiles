import { rm } from "node:fs/promises";

await rm("./dist", { recursive: true, force: true });
const result = await Bun.build({ entrypoints: ["./index.html"], outdir: "./dist", minify: true });
if (!result.success) {
  for (const log of result.logs) {
    console.error(log);
  }
  process.exit(1);
}
