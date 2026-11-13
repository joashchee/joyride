// The kit registry and the coverage report. A kit is data: its subjects (taken from the kit's own document), the recipes that make
// each, and what is missing. `coverage()` makes every referenced clip and lints it, so a kit that points at a recipe that does not
// exist, or one that throws, fails here and not in front of a user. To add a kit: write its file, import it below.
import locomotion from './locomotion.js';
import { animate, RECIPES } from '../recipes/index.js';
import { lintClip } from '../lint.js';

export const KITS = [locomotion];
export const registerKit = (kit) => { if (KITS.some((k) => k.id === kit.id)) throw new Error(`kit "${kit.id}" already registered`); KITS.push(kit); };

/** covered | partial | gap | reference-only, plus problems found while making its recipes. */
export function subjectStatus(subject) {
  const problems = [], warnings = [];
  if (subject.reference) return { status: 'reference-only', problems, warnings };
  for (const { recipe, params, lint: rules = {} } of subject.recipes ?? []) {
    if (!RECIPES[recipe]) { problems.push(`unknown recipe "${recipe}"`); continue; }
    try { lintClip(animate(recipe, params), rules).forEach((x) => (x.level === 'error' ? problems : warnings).push(x.message)); }
    catch (e) { problems.push(`${recipe} ${JSON.stringify(params)} threw: ${e.message}`); }
  }
  const has = subject.recipes?.length > 0;
  return { status: !has ? 'gap' : subject.partial ? 'partial' : 'covered', problems, warnings: [...new Set(warnings)] };
}

export function coverage(kits = KITS) {
  return kits.map((kit) => {
    const rows = kit.subjects.map((s) => ({ id: s.id, label: s.label, note: s.partial ?? s.reference ?? s.gap ?? '', ...subjectStatus(s) }));
    const count = (st) => rows.filter((r) => r.status === st).length;
    return { id: kit.id, title: kit.title, doc: kit.doc, rows, totals: { covered: count('covered'), partial: count('partial'), gap: count('gap'), 'reference-only': count('reference-only') } };
  });
}
