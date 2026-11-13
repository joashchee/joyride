// The kit registry and the coverage report. A kit is data: its subjects (taken from the kit's own document), the recipes that
// draw each, and what is missing. `coverage()` draws every referenced recipe, so a kit that points at a recipe that does not
// exist, or one that throws, fails here and not in front of a user. To add a kit: write its file, import it below.
import learning from './pixel-art-learning.js';
import faceart from './faceart.js';
import items from './items-equipment.js';
import biomes from './biomes-environments.js';
import motion from './hands-poses-motion-interiors.js';
import ansiArt from './ansi-art.js';
import couplerPainter from './coupler-painter.js';
import coffeemud from './coffeemud.js';
import freeAssets from './free-asset-sources.js';
import paperDoll from './paper-doll-game.js';
import splashScreens from './splash-screens.js';
import { draw, RECIPES, isNative } from '../recipes/index.js';
import { lint, lintFrames } from '../lint.js';
import { lintAnsi, lintAnsiFrames } from '../ansi-lint.js';

export const KITS = [learning, faceart, items, biomes, motion, ansiArt, couplerPainter, coffeemud, freeAssets, paperDoll, splashScreens];
export const registerKit = (kit) => { if (KITS.some((k) => k.id === kit.id)) throw new Error(`kit "${kit.id}" already registered`); KITS.push(kit); };

/** covered | partial | gap | built-in | reference-only, plus problems found while drawing its recipes. */
export function subjectStatus(subject) {
  const problems = [], warnings = [];
  if (subject.reference) return { status: 'reference-only', problems, warnings };
  for (const { recipe, params, style, lint: rules = {} } of subject.recipes ?? []) {
    if (!RECIPES[recipe]) { problems.push(`unknown recipe "${recipe}"`); continue; }
    try {
      if (style === 'ansi') {
        const a = draw(recipe, params, { style });
        a.frames.forEach((f, i) => lintAnsi(f, { name: `${recipe}#${i}`, ...rules }).forEach((x) => (x.level === 'error' ? problems : warnings).push(x.message)));
        if (a.clips) for (const [n, c] of Object.entries(a.clips)) lintAnsiFrames(c.frames, { name: `${recipe}.${n}`, fps: c.fps ?? a.fps, loop: c.loop }).forEach((x) => (x.level === 'error' ? problems : warnings).push(x.message));
        continue;
      }
      const a = draw(recipe, params);
      a.frames.forEach((f, i) => lint(f, { name: `${recipe}#${i}`, maxColors: 20, skip: ['tile', 'scene', 'overlay', 'telegraph', 'target', 'skillicon', 'splash'].includes(recipe) ? ['size', 'margin', 'outline', 'value'] : recipe === 'room' ? ['size', 'margin', 'outline', 'value', 'palette', 'noise'] : [] }).forEach((x) => (x.level === 'error' ? problems : warnings).push(x.message)));
      if (a.clips) for (const [n, c] of Object.entries(a.clips)) lintFrames(c.frames, { name: `${recipe}.${n}`, loop: c.loop, boil: recipe !== 'cloth' }).forEach((x) => warnings.push(x.message));
    } catch (e) { problems.push(`${recipe} ${JSON.stringify(params)} threw: ${e.message}`); }
  }
  const has = subject.recipes?.length > 0;
  const status = !has && subject.provides ? (subject.partial ? 'partial' : 'built-in') : !has ? 'gap' : subject.partial ? 'partial' : 'covered';
  return { status, problems, warnings: [...new Set(warnings)] };
}

/** How a subject's recipes draw in ANSI: `native` (all have a drawing made for cells), `folded` (none: the pixel drawing, folded), `mixed`, or '' for no recipes. */
function ansiKind(subject) {
  const ids = [...new Set((subject.recipes ?? []).filter((x) => RECIPES[x.recipe]).map((x) => x.recipe))];
  if (!ids.length) return '';
  const native = ids.filter((id) => isNative(id, 'ansi')).length;
  return native === ids.length ? 'native' : native ? 'mixed' : 'folded';
}

export function coverage(kits = KITS) {
  return kits.map((kit) => {
    const rows = kit.subjects.map((s) => ({ id: s.id, label: s.label, note: s.partial ?? s.reference ?? s.provides ?? s.gap ?? '', ansi: ansiKind(s), ...subjectStatus(s) }));
    const count = (st) => rows.filter((r) => r.status === st).length;
    return { id: kit.id, title: kit.title, doc: kit.doc, rows, totals: { covered: count('covered'), partial: count('partial'), 'built-in': count('built-in'), gap: count('gap'), 'reference-only': count('reference-only') } };
  });
}
