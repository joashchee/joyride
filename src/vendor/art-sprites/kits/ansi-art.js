// Models docs/cc0-ansi-art-learning-reference-kit.md. Posture: study material (tutorials, archives, editors); nothing from it is
// bundled, traced or used as a fixture, and the pieces here are drawn by the recipes in recipes/ansi.js from a seed.
// Subjects are the kit's five practice briefs, plus block lettering and the weather overlays; the rest are the work list.
const r = (recipe, params = {}, lint = {}) => ({ recipe, params, style: 'ansi', lint });
// Lettering is not a figure: the outline rule (a dark contour around a shape on an empty ground) does not apply to it.
const letters = (...faces) => faces.map((face) => r('banner', { face }, { skip: ['outline'] }));
export default {
  id: 'ansi-art',
  title: 'CC0 ANSI Art Learning Reference Kit',
  doc: 'docs/cc0-ansi-art-learning-reference-kit.md',
  workflow: ['Eyes or focal point first: it sets the scale', 'A single-line outline in dark gray before black', 'Base colour blocks', 'Highlights, then shadows', 'Blend last, keeping most of the outline'],
  subjects: [
    { id: 'logo', label: 'Logo: a short title in a few colours (brief 1)', recipes: letters('block', 'outlined') },
    { id: 'menu-screen', label: 'Menu screen with a box-drawing frame (brief 2)', recipes: [r('menu'), r('menu', { title: 'QUEST', items: ['Start', 'Continue', 'Quit'], selected: 1 })] },
    { id: 'shaded-landscape', label: 'Landscape with a shaded sky (brief 3)', recipes: ['day', 'dusk', 'night'].map((time) => r('landscape', { time })), partial: 'one composition (sky, two ranges, lake, ground); no towns, trees or clouds' },
    { id: 'halfblock-portrait', label: 'Half-block portrait with three-value lighting (brief 4)', recipes: [r('bust'), r('bust', { skin: 'pale', hair: 'gray', light: 'right' })], partial: 'a front-facing bust; no expressions or angles' },
    { id: 'artpack-piece', label: 'An original artpack piece with SAUCE (brief 5)', recipes: [r('artpack'), r('artpack', { title: 'NIGHT', time: 'night', weather: 'snow' })] },
    { id: 'block-lettering', label: 'Block lettering for titles, logos and menus', recipes: letters('half', 'block', 'outlined', 'shaded'), partial: 'one 3 by 5 font in four faces; capitals, digits and four marks only' },
    { id: 'glyph-overlays', label: 'Stars, rain, snow, waves and runes as glyph overlays', recipes: [r('landscape', { time: 'night' }), r('landscape', { weather: 'rain' }), r('landscape', { weather: 'snow' }), r('menu')] },
    { id: 'ansimation', label: 'ANSImation: frames as cursor-move deltas', recipes: [r('weatherloop', { weather: 'rain' }), r('weatherloop', { weather: 'snow' })], partial: 'falling weather only; no character or scene animation written natively' },
    { id: 'image-to-cells', label: 'Converting a reference image to cells for study', provides: 'fromImage in ansi-image.js (and `sprites convert`): a drafting aid for study, not a recipe; its output is not engine art' },
    { id: 'reference-tutorials', label: 'Shading, curving and colour-science tutorials; font tutorials; archived .ANS packs', reference: 'tutorials and archives are not CC0: read by a person in an ANSI viewer; never bundled, traced or used as test files' },
  ],
};
