// Models docs/cc0-splash-screen-reference-kit.md. Posture: permissive sources only (CC0 verified on the page, or marked unverified);
// nothing from them is bundled, traced or used as a fixture. The pictures are drawn by recipes/splash.js from a title, a scene and a seed.
const SCENES = ['meadow', 'forest', 'desert', 'mountains', 'sea', 'snow', 'night'];
const px = (scene, title = 'Hollow Keep') => ({ recipe: 'splash', params: { title, scene } });
const an = (scene, title = 'Hollow Keep') => ({ recipe: 'splash', params: { title, scene }, style: 'ansi', lint: { skip: ['ramp'] } });
export default {
  id: 'splash-screens',
  title: 'Splash Screen Reference Kit (pixel and ANSI)',
  doc: 'docs/cc0-splash-screen-reference-kit.md',
  workflow: ['Title first: one focal point, margin on every side', 'A dark plate under the letters', 'Sky in dithered bands, then the light, then ridges far to near', 'Keep the scene quieter than the title', 'Judge all scenes on one sheet'],
  subjects: [
    { id: 'splash-pixel', label: 'Pixel splash: a title over a scene, 256 by 144 on DB32', recipes: SCENES.map((s) => px(s)), partial: 'seven scenes (land, sea, snow, night): no town, castle, ship or figure; no subtitle, credit or prompt line (a game sets those); a still, no fade' },
    { id: 'splash-ansi', label: 'ANSI splash: block lettering over a vista, 80 by 24, classic-legal', recipes: SCENES.map((s) => an(s)), partial: 'one vista per scene; a still, no ansimation' },
    { id: 'splash-title-lengths', label: 'Short and long titles: the lettering steps down from shaded to block to half', recipes: [an('night', 'Q'), an('meadow', 'The Long Way Home'), px('snow', 'The Long Way Home')] },
    { id: 'splash-sources', label: 'Permissive sources: IAMST title pack, Kenney UI and backgrounds, OpenGameArt CC0 backgrounds', reference: 'CC0 verified on the page for IAMST and Kenney; OpenGameArt rows unverified (page not fetchable). Never bundled, traced or used as a fixture. Excluded: CC-BY items, BBS ANSI, GPL generators, all-rights-reserved articles' },
  ],
};
