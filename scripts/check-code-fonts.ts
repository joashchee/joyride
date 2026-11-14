// Checks the coding fonts against their files: each exists, is a real font, and has the advance and units per em the catalogue says
// (read from the file's own head, hhea and hmtx tables); each has a licence file; NOTICE names each; and the sizes offered come out
// whole-pixel (a bitmap font only at multiples of its native size). Run: node scripts/check-code-fonts.ts
import { readFileSync, existsSync } from "node:fs";
import { CODE_FONTS, cellFor, cleanSizes, TOLERANCE } from "../src/lib/codefonts.ts";

let bad = 0;
const fail = (m: string) => { bad++; console.log("FAIL " + m); };

function metrics(path: string) {
  const b = readFileSync(path);
  const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const tag = dv.getUint32(0);
  if (tag !== 0x00010000 && tag !== 0x4f54544f) throw new Error("not a TrueType or OpenType font");
  const n = dv.getUint16(4);
  const t: Record<string, number> = {};
  for (let i = 0; i < n; i++) t[b.toString("latin1", 12 + i * 16, 16 + i * 16)] = dv.getUint32(20 + i * 16);
  const upem = dv.getUint16(t.head + 18);
  const nm = dv.getUint16(t.hhea + 34);
  const count = new Map<number, number>();
  for (let i = 0; i < nm; i++) { const a = dv.getUint16(t.hmtx + i * 4); if (a) count.set(a, (count.get(a) ?? 0) + 1); }
  const advance = [...count].sort((x, y) => y[1] - x[1])[0][0];
  return { upem, advance };
}

const notice = readFileSync("NOTICE", "utf8");
for (const f of CODE_FONTS) {
  const path = `public/fonts/code/${f.file}`;
  if (!existsSync(path)) { fail(`${f.id}: ${path} missing`); continue; }
  try {
    const m = metrics(path);
    if (m.upem !== f.upem || m.advance !== f.advance) fail(`${f.id}: file says advance ${m.advance} / upem ${m.upem}, catalogue says ${f.advance} / ${f.upem}`);
  } catch (e) { fail(`${f.id}: ${e}`); }
  const lic = `public/fonts/code/licences/${f.file.replace(/-\d+x\d+\.otf$/, "").replace(/\.(ttf|otf)$/, "")}.txt`;
  if (!existsSync(lic) || readFileSync(lic, "utf8").length < 500) fail(`${f.id}: licence file ${lic} missing or short`);
  if (!notice.includes(f.name.replace(/ \d+x\d+$/, ""))) fail(`${f.id}: NOTICE does not name ${f.name}`);
  for (const dpr of [1, 2]) {
    const sizes = cleanSizes(f, dpr);
    if (sizes.length < (f.kind === "bitmap" ? 1 : 3)) fail(`${f.id}: only ${sizes.length} clean sizes at ${dpr}x`);
    for (const s of sizes) {
      const c = cellFor(f, s, dpr);
      if (c.drift > TOLERANCE + 1e-9 && f.kind === "vector") fail(`${f.id} ${s}px: drift ${c.drift}`);
      if (Math.abs(c.width * dpr - Math.round(c.width * dpr)) > 1e-9) fail(`${f.id} ${s}px: cell width ${c.width} is not whole device pixels`);
      if (f.kind === "bitmap" && s % f.native! !== 0) fail(`${f.id}: ${s}px is not a multiple of ${f.native}`);
    }
  }
}
// The sizes a laptop and a large display get, so a font with nothing in either range is noticed.
for (const f of CODE_FONTS) {
  const s = cleanSizes(f, 1);
  if (!s.some((x) => x <= 16 || f.kind === "bitmap") || !s.some((x) => x >= 24)) fail(`${f.id}: no laptop size or no large size (${s})`);
}
console.log(bad ? `${bad} problem(s)` : `ok: ${CODE_FONTS.length} fonts match their files; sizes at 1x: ` + CODE_FONTS.map((f) => `${f.id} ${cleanSizes(f, 1).join(",")}`).join("; "));
process.exit(bad ? 1 : 0);
