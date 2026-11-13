// The kit registry and the coverage report. A kit is data: its subjects, the recipes that compose each, and what is missing.
// `coverage()` composes every referenced recipe, so a kit that points at a recipe that does not exist, or one that throws, fails
// here and not in front of a user. To add a kit: write its file, import it below.
import gameMusic from './game-music.js';
import songCraft from './song-craft.js';
import { compose, RECIPES } from '../recipes/index.js';
import { lint } from '../lint.js';

export const KITS = [gameMusic, songCraft];
export const registerKit = (kit) => { if (KITS.some((k) => k.id === kit.id)) throw new Error(`kit "${kit.id}" already registered`); KITS.push(kit); };

/** covered | partial | gap | built-in | reference-only, plus problems found while composing its recipes. */
// The engines a row may name: Neumetik's sound makers it is meant to be heard through (docs/composer-plan.md).
export const KIT_ENGINES = ['midi', 'opl', 'sample', 'bgn', 'chipbank', 'chip'];

export function subjectStatus(subject) {
  const problems = [], warnings = [];
  if (subject.engine && !KIT_ENGINES.includes(subject.engine)) problems.push(`unknown engine "${subject.engine}"; known: ${KIT_ENGINES.join(', ')}`);
  if (subject.reference) return { status: 'reference-only', problems, warnings };
  for (const { recipe, params } of subject.recipes ?? []) {
    if (!RECIPES[recipe]) { problems.push(`unknown recipe "${recipe}"`); continue; }
    try {
      const a = compose(recipe, params);
      lint(a.score, { name: `${recipe} ${JSON.stringify(params)}`, loop: a.loop, ...a.lint }).forEach((x) => (x.level === 'error' ? problems : warnings).push(x.message));
    } catch (e) { problems.push(`${recipe} ${JSON.stringify(params)} threw: ${e.message}`); }
  }
  const has = subject.recipes?.length > 0;
  const status = !has && subject.provides ? (subject.partial ? 'partial' : 'built-in') : !has ? 'gap' : subject.partial ? 'partial' : 'covered';
  return { status, problems, warnings: [...new Set(warnings)] };
}

export function coverage(kits = KITS) {
  return kits.map((kit) => {
    const rows = kit.subjects.map((s) => ({ id: s.id, label: s.label, engine: s.engine ?? '', note: s.partial ?? s.reference ?? s.provides ?? '', ...subjectStatus(s) }));
    const count = (st) => rows.filter((r) => r.status === st).length;
    return { id: kit.id, title: kit.title, doc: kit.doc, rows, totals: { covered: count('covered'), partial: count('partial'), 'built-in': count('built-in'), gap: count('gap'), 'reference-only': count('reference-only') } };
  });
}
