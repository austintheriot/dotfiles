import { deflateSync } from "node:zlib";

const size = 128;
const raw = new Uint8Array(size * (size * 4 + 1));
for (let row = 0; row < size; row++) {
  const start = row * (size * 4 + 1);
  for (let column = 0; column < size; column++) {
    const inside = (column - 63.5) ** 2 + (row - 63.5) ** 2 <= 60 ** 2;
    raw.set(inside ? [0xe0, 0xa0, 0x30, 0xff] : [0, 0, 0, 0], start + 1 + column * 4);
  }
}
const chunk = (type: string, data: Uint8Array): Uint8Array => {
  const body = new Uint8Array([...new TextEncoder().encode(type), ...data]);
  const out = new DataView(new ArrayBuffer(12 + data.length));
  out.setUint32(0, data.length);
  new Uint8Array(out.buffer).set(body, 4);
  out.setUint32(8 + data.length, Bun.hash.crc32(body));
  return new Uint8Array(out.buffer);
};
const header = new DataView(new ArrayBuffer(13));
header.setUint32(0, size);
header.setUint32(4, size);
new Uint8Array(header.buffer).set([8, 6, 0, 0, 0], 8);
const png = [
  new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", new Uint8Array(header.buffer)),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", new Uint8Array()),
];
await Bun.write(process.argv[2] ?? "icon.png", new Blob(png));
