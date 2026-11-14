// Checks the window snapping sums. Run: node scripts/check-snap.ts
import { snapMoves, snapDelta } from "../src/lib/snap.ts";
import { cellFor, CODE_FONTS, cleanSizes } from "../src/lib/codefonts.ts";

let bad = 0, n = 0;
const eq = (got: unknown, want: unknown, what: string) => { n++; if (got !== want) { bad++; console.log(`FAIL ${what}: got ${got}, wanted ${want}`); } };

// 9 px cells in a 1000 px text area: 1000 = 111 * 9 + 1, so one 8 px shell cell wider makes 1008 = 112 * 9.
eq(snapMoves(1000, 9, 8), 1, "9px cells, 1000 wide");
eq(snapMoves(1008, 9, 8), 0, "already whole");
eq(snapMoves(1001, 9, 8), 2, "1001 -> 1017 is 113 cells (two cells wider is nearer than seven narrower)");
// 12 px cells and 8 px steps only line up on multiples of 4: 1001 never does.
eq(snapMoves(1001, 12, 8), null, "no way to line up");
eq(snapMoves(1000, 12, 8), 1, "12px cells, 1000 -> 1008 = 84 cells");
// Never snap to a tiny editor.
eq(snapMoves(30, 10, 10, 20), 17, "the smallest allowed is 20 cells");
// Lines: 16 px steps, 19 px lines.
eq((700 + 16 * snapMoves(700, 19, 16)!) % 19, 0, "lines line up");
// A whole sweep: every clean size of every font, at 1x and 2x, can be reached from any width, or says it cannot.
for (const f of CODE_FONTS) for (const dpr of [1, 2]) for (const s of cleanSizes(f, dpr)) {
  const c = cellFor(f, s, dpr);
  const cellW = Math.round(c.width * dpr), cellH = Math.round(c.lineHeight * dpr);
  for (let content = 20 * cellW; content < 20 * cellW + 40; content++) {
    const k = snapMoves(content, cellW, 8 * dpr);
    if (k === null) continue;
    n++;
    if ((content + k * 8 * dpr) % cellW !== 0) { bad++; console.log(`FAIL ${f.id} ${s}px@${dpr}x width ${content}`); break; }
  }
  const d = snapDelta({ contentW: 1000 * dpr, contentH: 600 * dpr, cellW, cellH, stepW: 8 * dpr, stepH: 16 * dpr }, dpr);
  eq(Number.isFinite(d.dw) && Number.isFinite(d.dh), true, `${f.id} ${s}px delta is finite`);
}
console.log(bad ? `${bad} failed of ${n}` : `ok: ${n} snap checks`);
process.exit(bad ? 1 : 0);
