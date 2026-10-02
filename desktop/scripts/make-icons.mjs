// Draws the app's icons, without any image tools: build/icon.ico + build/icon.png (Agent Office, an
// office building) and claude/claude-code.ico (the "Claude Code (ohne API-Key)" shortcut, a prompt).
// Run it again after changing a drawing: node desktop/scripts/make-icons.mjs
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

const desktop = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const SIZES = [16, 24, 32, 48, 64, 128, 256];
const SS = 4; // supersampling per axis

const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];

/** Inside a rounded rectangle (all in 0..1 units). */
function inRound(x, y, x0, y0, x1, y1, r) {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  return (x - cx) ** 2 + (y - cy) ** 2 <= r * r;
}
const inRect = (x, y, x0, y0, x1, y1) => x >= x0 && x <= x1 && y >= y0 && y <= y1;
/** Within distance w/2 of the segment a–b. */
function onLine(x, y, ax, ay, bx, by, w) {
  const dx = bx - ax;
  const dy = by - ay;
  const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)));
  return (x - ax - t * dx) ** 2 + (y - ay - t * dy) ** 2 <= (w / 2) ** 2;
}

/** A color for the point (x, y) in 0..1, or null for transparent. */
const DRAWINGS = {
  office(x, y) {
    if (!inRound(x, y, 0.04, 0.04, 0.96, 0.96, 0.2)) return null;
    // The building, its roof line and a grid of lit windows.
    if (inRect(x, y, 0.27, 0.22, 0.73, 0.84)) {
      if (inRect(x, y, 0.44, 0.72, 0.56, 0.84)) return hex('#2b2f4a');
      const col = Math.floor((x - 0.33) / 0.12);
      const row = Math.floor((y - 0.3) / 0.12);
      const fx = (x - 0.33) - col * 0.12;
      const fy = (y - 0.3) - row * 0.12;
      if (col >= 0 && col < 3 && row >= 0 && row < 3 && fx < 0.08 && fy < 0.08) return hex(row === 1 && col === 1 ? '#d97757' : '#ffd479');
      return hex('#f4f1ea');
    }
    if (inRect(x, y, 0.2, 0.84, 0.8, 0.88)) return hex('#f4f1ea');
    const t = y;
    return [Math.round(58 + 30 * t), Math.round(63 + 20 * t), Math.round(140 - 20 * t)];
  },
  claude(x, y) {
    if (!inRound(x, y, 0.04, 0.04, 0.96, 0.96, 0.2)) return null;
    // ">_" in a console.
    if (onLine(x, y, 0.26, 0.32, 0.48, 0.5, 0.11) || onLine(x, y, 0.48, 0.5, 0.26, 0.68, 0.11)) return hex('#ffffff');
    if (inRect(x, y, 0.53, 0.63, 0.76, 0.72)) return hex('#ffffff');
    return hex('#d97757');
  },
};

function render(draw, size) {
  const px = Buffer.alloc(size * size * 4);
  for (let j = 0; j < size; j++) {
    for (let i = 0; i < size; i++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let sj = 0; sj < SS; sj++) {
        for (let si = 0; si < SS; si++) {
          const c = draw((i + (si + 0.5) / SS) / size, (j + (sj + 0.5) / SS) / size);
          if (!c) continue;
          r += c[0];
          g += c[1];
          b += c[2];
          a++;
        }
      }
      const o = (j * size + i) * 4;
      if (a) {
        px[o] = Math.round(r / a);
        px[o + 1] = Math.round(g / a);
        px[o + 2] = Math.round(b / a);
      }
      px[o + 3] = Math.round((255 * a) / (SS * SS));
    }
  }
  return px;
}

const CRC = new Int32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c;
});
function crc32(buf) {
  let c = -1;
  for (const byte of buf) c = CRC[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
function png(rgba, size) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) rgba.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw, { level: 9 })), chunk('IEND', Buffer.alloc(0))]);
}
/** An .ico holding a PNG per size (Windows Vista and later read those). */
function ico(pngs) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(pngs.length, 4);
  const dir = Buffer.alloc(16 * pngs.length);
  let offset = 6 + dir.length;
  pngs.forEach(({ size, data }, n) => {
    const e = n * 16;
    dir[e] = size >= 256 ? 0 : size;
    dir[e + 1] = size >= 256 ? 0 : size;
    dir.writeUInt16LE(1, e + 4); // planes
    dir.writeUInt16LE(32, e + 6); // bits per pixel
    dir.writeUInt32LE(data.length, e + 8);
    dir.writeUInt32LE(offset, e + 12);
    offset += data.length;
  });
  return Buffer.concat([header, dir, ...pngs.map((p) => p.data)]);
}

function write(file, data) {
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, data);
  console.log(`wrote ${path.relative(desktop, file)}`);
}

const pngsOf = (draw) => SIZES.map((size) => ({ size, data: png(render(draw, size), size) }));
const office = pngsOf(DRAWINGS.office);
write(path.join(desktop, 'build', 'icon.ico'), ico(office));
write(path.join(desktop, 'build', 'icon.png'), png(render(DRAWINGS.office, 512), 512));
write(path.join(desktop, 'claude', 'claude-code.ico'), ico(pngsOf(DRAWINGS.claude)));
