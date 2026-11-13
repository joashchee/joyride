/** A colour value in a `Pixels` canvas: 0 is transparent, any other value picks `palette[value - 1]`. */
export type Color = number;
export type Palette = readonly string[];

export class Pixels {
  w: number; h: number; palette: Palette; data: Uint8Array;
  constructor(w: number, h: number, palette?: Palette);
  static fromRows(rows: string[], legend: Record<string, Color>, palette?: Palette): Pixels;
  static fromRgba(w: number, h: number, rgba: Uint8Array | Uint8ClampedArray): Pixels;
  inb(x: number, y: number): boolean;
  get(x: number, y: number): Color;
  set(x: number, y: number, c: Color): this;
  clone(): Pixels;
  equals(o: Pixels): boolean;
  thick(x0: number, y0: number, x1: number, y1: number, c: Color, thickness?: number): this;
  frame(x: number, y: number, w: number, h: number, c: Color): this;
  ellipse(cx: number, cy: number, rx: number, ry: number, c: Color): this;
  rect(x: number, y: number, w: number, h: number, c: Color): this;
  line(x0: number, y0: number, x1: number, y1: number, c: Color): this;
  poly(points: [number, number][], c: Color): this;
  fill(x: number, y: number, c: Color): this;
  swap(from: Color | Color[], to: Color | Color[]): this;
  blit(src: Pixels, dx?: number, dy?: number): this;
  outline(c: Color, diagonals?: boolean): this;
  map(fn: (x: number, y: number) => Color): Pixels;
  flipX(): Pixels; flipY(): Pixels; rot90(): Pixels;
  shift(dx: number, dy: number): Pixels;
  silhouette(c: Color): Pixels;
  bbox(): { x: number; y: number; w: number; h: number } | null;
  count(): number;
  colors(): Color[];
  hex(v: Color): string;
  luma(v: Color): number;
  /** RGBA bytes at an integer scale, over `bg` (hex) or transparent. */
  toRgba(scale?: number, bg?: string | null): Uint8Array;
}

export interface Clip { id?: string; frames: Pixels[]; fps: number; loop: boolean; events?: any[]; reduced?: Pixels[]; anchors?: Record<string, any>[] }

/** What every recipe returns. */
export interface Art<F = Pixels> {
  id: string;
  frames: F[];
  /** `ansi` on a drawing made in that style; absent on pixel drawings. */
  style?: Style;
  /** True when an `ansi` drawing is a pixel drawing folded to cells. */
  folded?: boolean;
  fps: number;
  loop: boolean;
  /** Named animations (idle, attack, hit, ...), for recipes that have them. */
  clips?: Record<string, Clip>;
  /** Separately drawn parts of a layered character, for a game that composes or recolours them itself. */
  layers?: Record<string, Pixels[]>;
  /** Named points in the canvas: `root`, `shadow_center`, `head`, `effect_origin` on every pixel drawing; a character adds `neck`, `shoulder_left/right`, `hand_main`, `hand_off`, `waist`, `back`, `foot_left/right`, `weapon_grip`, `shield_grip`, and `frames` (one set per frame). A clip's `anchors` is one set per frame. */
  anchors?: Record<string, any>;
  /** A short factual phrase for a screen reader. Every drawing from `draw()` has one. */
  alt?: string;
  /** The box the drawing fills (pixel art). */
  bounds?: { x: number; y: number; w: number; h: number };
  /** The still to show instead of an animation. Every drawing that animates, and every clip, has one. */
  reduced?: Frame[];
  /** How the world is seen: `front` for characters and creatures. Absent where a recipe has not said. */
  projection?: string;
  /** Which ramp each colour group uses (character, creature): the handles `dye` and `recolor` work with. */
  paletteGroups?: Record<string, string>;
  [extra: string]: any;
}
type Frame = any;

/** A drawing language: `pixel` (frames are Pixels) or `ansi` (frames are Cells). */
export type Style = 'pixel' | 'ansi' | 'text';

/** No colour in a cell: the viewer's own background, or what is below it. */
export const NONE: 0xffff;
export const BOLD: 1, BLINK: 2;
/** `fgRgb` and `bgRgb` are there only on a picture that has true colour (`Cells.tc`): 0xRRGGBB, or -1 where the palette index stands. */
export interface Cell { ch: number; fg: number; bg: number; flags: number; fgRgb?: number; bgRgb?: number }
/** What a picture drawn with `colors: 'true'` carries beside its palette indexes: the exact colours, 0xRRGGBB, -1 where the index stands. The indexes hold the nearest of the 256, for a viewer that cannot show true colour. */
export interface TrueColor { fg: Int32Array; bg: Int32Array }
/** The ANSI grid: a character and two palette INDEXES (0 to 255) a cell. */
export class Cells {
  constructor(w: number, h: number);
  w: number; h: number;
  ch: Uint32Array; fg: Uint16Array; bg: Uint16Array; flags: Uint8Array;
  /** The true-colour layer, or null: a 16 or 256-colour picture has none. */
  readonly tc: TrueColor | null;
  /** The true-colour layer, made when there is none. */
  trueColor(): TrueColor;
  inb(x: number, y: number): boolean;
  get(x: number, y: number): Cell;
  /** `fgRgb` and `bgRgb` (0xRRGGBB) are exact colours for a true-colour viewer; a colour set by index alone clears its RGB. */
  set(x: number, y: number, cell?: { ch?: string | number; fg?: number; bg?: number; flags?: number; fgRgb?: number; bgRgb?: number }): this;
  isEmpty(x: number, y: number): boolean;
  rect(x: number, y: number, w: number, h: number, cell?: { ch?: string | number; fg?: number; bg?: number; flags?: number }): this;
  fill(cell?: { ch?: string | number; fg?: number; bg?: number; flags?: number }): this;
  text(x: number, y: number, str: string, fg?: number, bg?: number): this;
  glyph(x: number, y: number, ch: string | number, fg?: number): this;
  blit(src: Cells, dx?: number, dy?: number): this;
  /** Box-drawing lines; crossing or ending on another line makes the right join (`├ ┼ ┬ ╟ ╪` ...). */
  hline(x: number, y: number, len: number, opts?: BoxOptions): this;
  vline(x: number, y: number, len: number, opts?: BoxOptions): this;
  /** `mixed` is double horizontals with single verticals. Throws under 2 by 2. */
  box(x: number, y: number, w: number, h: number, opts?: BoxOptions): this;
  /** A rectangle of ` `, `░`, `▒`, `▓` or `█`. */
  shadeFill(x: number, y: number, w: number, h: number, glyph: ' ' | '░' | '▒' | '▓' | '█', fg?: number, bg?: number): this;
  clone(): Cells;
  equals(o: Cells): boolean;
  toText(): string;
}
export interface BoxOptions { style?: 'single' | 'double' | 'mixed'; fg?: number; bg?: number }
/** Background colours a target allows: 8 for a classic `.ans` (bright ones are the blink bit), 16 for iCE, UTF-8 and modern terminals. */
export type BgLimit = 8 | 16;
/** A pixel canvas folded to cells: two square pixels a cell, as half blocks. Legal for `bgLimit` (default 8). */
export function foldPixels(px: Pixels, opts?: { map?: readonly number[]; bgLimit?: BgLimit }): Cells;
export function foldArt(art: Art, opts?: { map?: readonly number[]; bgLimit?: BgLimit }): Art<Cells>;
/** The colours of a cell's upper and lower halves, as [r, g, b] each. */
export function halves(cell: { ch: number; fg: number; bg: number; fgRgb?: number; bgRgb?: number }): [number[], number[]];
/** A cell's effective colour as `#rrggbb`: `░ ▒ ▓` mix 25, 50, 75 percent of fg over bg, half blocks average their halves. NONE shows as black. */
export function tone(cell: { ch: number; fg: number; bg: number; fgRgb?: number; bgRgb?: number }): string;
/** One cell made legal for `bgLimit`: reversed (`▀`/`▄`, `░`/`▓`) where that keeps the picture, else the nearest legal cell. */
export function legalCell(cell: Cell, bgLimit?: BgLimit): Cell;
/** A copy with every background legal for `bgLimit`; a plain copy at 16. Backgrounds from 16 up and NONE are left alone. */
export function legalize(cells: Cells, opts?: { bgLimit?: BgLimit }): Cells;
/** The five steps `a`, `░ ▒ ▓` of `b` over `a`, `b`, each legal for `bgLimit`. */
export function ladder(a: number, b: number, opts?: { bgLimit?: BgLimit }): Cell[];
/** A ramp (a name from `ANSI_RAMPS` or colour indexes, dark to light) as one run of ladder steps. */
export function rampCells(ramp: string | readonly number[], opts?: { bgLimit?: BgLimit }): Cell[];
export function colorMap(palette: Palette, map?: readonly number[]): (value: number) => number;

/** IBM code page 437: the Unicode code point of each byte (0 is taken as a space). */
export const CP437: readonly number[];
export type AnsiTarget = 'classic' | 'ice' | 'modern';
export interface SauceOptions {
  title?: string; author?: string; group?: string;
  /** `YYYYMMDD`. Never read from a clock: the default is `00000000`, so the same cells give the same bytes. */
  date?: string;
  /** Font name, default `IBM VGA`. */
  font?: string; comments?: string[];
  /** Default `8`, the standard font's cell. */
  letterSpacing?: 'none' | '8' | '9'; aspect?: 'none' | 'legacy' | 'square';
}
export interface Sauce extends Required<Omit<SauceOptions, 'letterSpacing' | 'aspect'>> {
  dataType: number; fileType: number; fileSize: number; width: number; height: number; ice: boolean;
  letterSpacing: 'none' | '8' | '9'; aspect: 'none' | 'legacy' | 'square';
}
export interface AnsiOutput {
  bytes: Uint8Array; charset: 'cp437' | 'utf8'; target: AnsiTarget; colors: 16 | 256 | 'true';
  /** Whether the file uses bright backgrounds as the blink bit, and says so in SAUCE. */
  ice: boolean; sauce: boolean; width: number; height: number;
  /** Everything that had to change (depth reduced, characters not in CP437, blink dropped); never silent. */
  warnings: string[];
  /** The stream alone as a string, for UTF-8. */
  text?: string;
}
/** Cells to the bytes of an ANSI file: CRLF lines, an escape only when attributes change, Ctrl-Z and SAUCE. `classic` (CP437, 8 backgrounds) by default. */
export function writeAnsi(cells: Cells, opts?: { target?: AnsiTarget; charset?: 'cp437' | 'utf8'; colors?: 16 | 256 | 'true'; sauce?: boolean | SauceOptions }): AnsiOutput;
export interface AnsiRead { cells: Cells; width: number; height: number; sauce: Sauce | null; charset: 'cp437' | 'utf8'; ice: boolean; warnings: string[] }
/** An ANSI file (bytes, or a string) to cells. Width is SAUCE's, else `width`, else 80; capped at 400 by 1000. */
export function readAnsi(input: Uint8Array | string, opts?: { width?: number; charset?: 'auto' | 'cp437' | 'utf8'; ice?: boolean }): AnsiRead;
export interface RenderedCells { w: number; h: number; rgba: Uint8Array; /** Code points outside CP437, drawn as `?`. */ missing: number[] }
/** Cells to an RGBA image with the VGA glyph atlas (`loadAtlas` from `art-sprites/atlas`, or your own decode of docs/fonts/ibm-vga-8x16.png). 8 by 16 pixels a cell, or 9 by 16 with `letterSpacing: 9`. */
export function renderCells(cells: Cells, opts: { atlas: { w: number; h: number; rgba: Uint8Array }; theme?: readonly string[]; letterSpacing?: 8 | 9; bold?: boolean }): RenderedCells;

/** Fill a region with a gradient along a ramp (a name from `ANSI_RAMPS` or VGA indexes): the `░ ▒ ▓` ladders, legal for `bgLimit`. Throws under four cells along it. */
export function gradient(cells: Cells, x: number, y: number, w: number, h: number, ramp: string | readonly number[], opts?: { dir?: 'down' | 'up' | 'right' | 'left'; bgLimit?: BgLimit }): Cells;
/** An ordered (4 by 4 Bayer) dither: each cell solid `a` or `b`, the share of `b` following `level` (0 to 1, or per cell). No randomness. */
export function bayer(cells: Cells, x: number, y: number, w: number, h: number, a: number, b: number, level: number | ((x: number, y: number) => number)): Cells;
/** The in-between colour of every cell as `#rrggbb`, row by row: what a truecolor viewer paints for it. */
export function tones(cells: Cells): string[];
/** `░ ▒ ▓` cells become solid cells of their in-between colour, the nearest of the 256 (or the 16, legalized): for a 256-colour file. */
export function resolveTones(cells: Cells, opts?: { depth?: 16 | 256; bgLimit?: BgLimit }): Cells;
/** An image (`Pixels` or `{ w, h, rgba }`) to cells, for study only: contrast stretch, Lanczos resize, half blocks for the 2:1 cell, each cell the least-error glyph and colour pair (16) or the nearest of the 256. Not engine art. */
export function fromImage(image: Pixels | { w: number; h: number; rgba: Uint8Array | Uint8ClampedArray }, opts?: { columns?: number; colors?: 16 | 256; bgLimit?: BgLimit; contrast?: boolean }): { cells: Cells; warnings: string[] };
/** Ink from light to dark: a space is black, `@` is white. */
export const ASCII_RAMP: string;
/** A pixel canvas as ASCII cells: a cell's two pixels read as light and dark; one opaque pixel is `'` (upper) or `,` (lower). */
export function foldText(px: Pixels): Cells;
/** ANSI cells as ASCII: ASCII glyphs stay, box drawing becomes `- = | +`, blocks and shades become ink by brightness, other characters `*`. Colour dropped. */
export function asciiFromCells(cells: Cells): Cells;
export function textFromPixelArt(art: Art): Art<Cells>;
export function textFromAnsiArt(art: Art<Cells>): Art<Cells>;
/** Cells to 7-bit plain text: `?` and a warning for anything else, trailing spaces cut, `\n` or `\r\n` lines. */
export function writeText(cells: Cells, opts?: { eol?: '\n' | '\r\n' }): { text: string; bytes: Uint8Array; warnings: string[] };
/** Text-art findings: `ascii`, `width`, `height`, `blank`, `fill`, `colour`. */
export function lintText(cells: Cells, opts?: { name?: string; width?: number; maxHeight?: number; skip?: string[] }): { rule: string; level: 'error' | 'warn'; message: string }[];
/** CoffeeMUD's 24 terrains (14 outdoor, 10 indoor), and whether one has a sky (outdoors, not underwater). */
export const TERRAINS: readonly string[];
export function hasSky(terrain: string): boolean;
/** A room's seed as Coupler keeps it: FNV-1a (64 bit) of world, a zero byte, room and the look count (four little-endian bytes), as a decimal string for a `seed` param. */
export function roomSeed(world: string, room: string, again?: number): string;

/** The creature face (`creatureface`, ANSI only): head shape of a race or category by Coupler's rule, what a description says, the hurt step of a health line. */
export type FaceShape = 'Person' | 'Beast(Pointed)' | 'Beast(Round)' | 'Beast(Wide)' | 'Beast(Long)' | 'Beast(Horned)' | 'Beast(Curled)' | 'Bird' | 'Scaled(Lizard)' | 'Scaled(Snake)' | 'Scaled(Frog)' | 'Scaled(Fish)' | 'Scaled(Dragon)' | 'Bug(Insect)' | 'Bug(Spider)' | 'Bug(Worm)' | 'Skull' | 'Stuff(Fire)' | 'Stuff(Water)' | 'Stuff(Air)' | 'Stuff(Earth)' | 'Stuff(Golem)' | 'Stuff(Slime)' | 'Stuff(Plant)';
export interface FaceWords { skin?: number; hair?: number; eyes?: number; coat?: number; bald: boolean; beard: boolean; clean: boolean; mustache: boolean; scar: boolean; patch: boolean; freckles: boolean; tattoo: boolean; horns: boolean; tusks: boolean; fangs: boolean; glow: boolean; halo: boolean; old: boolean; longHair: boolean; kind: boolean; arms: boolean }
export function faceShape(race: string, category?: string): FaceShape;
export function readFace(params?: Record<string, unknown>): { race: string; shape: FaceShape; family: string; sapient: boolean; animal: boolean; skin: number; hair?: number; eyes: number; backdrop: number; mood: number; hurt: number; looks: string[]; flags: Record<string, boolean | undefined> };
export function readFaceWords(text: string): FaceWords;
/** The hurt step 0 to 5 for a step number, `{ health }` (a fraction left, 0 to 1) or CoffeeMUD health text. */
export function hurtStep(text: number | string | { health: number } | undefined): number;
export function hurtClass(face: { shape: string; family: string; ghostly?: boolean }): 'Bleed(RED)' | 'Bleed(GREEN)' | 'Mat' | 'Crack' | 'Fade' | 'Shrink';
/** The look an effect's name gives (160 names to 15 looks), or undefined. */
export function effectLook(name: string): string | undefined;
export const FACE_MOODS: readonly string[], FACE_SIZES: readonly string[], FACE_LOOKS: readonly string[], FACE_SHAPES: readonly FaceShape[], FACE_PARTS: readonly string[], HURT_STEPS: readonly string[];
/** The item picture (`thing`, ANSI only): the kind a name gives, the material's class, wear and aura words, and what a picture is read from. */
export type ThingClass = 'cloth' | 'glass' | 'leather' | 'metal' | 'mithril' | 'plain' | 'precious' | 'rock' | 'wooden';
export interface ThingWear { step: number; rust: boolean; crude: boolean; fine: boolean }
/** The kind a name gives: its first word that names one (a material word only if nothing else does); none gives a Bag for a container, else a Bundle. */
export function thingKind(name: string, opts?: { container?: boolean }): { kind: string; word?: string };
/** A material's class and colour: the material named, else a material word in the name before any "of". `cls` is the table's class, `draws` the texture it gets. */
export function materialClass(material: string | undefined, name?: string): { material: string; cls: string; color?: number; draws: ThingClass } | undefined;
/** The wear a name or param gives: a word, a list of words or an object. */
export function thingWear(wear?: string | string[] | Partial<ThingWear>, name?: string): ThingWear;
/** An aura word's glow colour (a VGA index), or undefined. */
export function auraGlow(aura: string): number | undefined;
export function readThing(params?: Record<string, unknown>): { name: string; kind: string; word?: string; cls: ThingClass; material?: string; main: number; wear: ThingWear; auras: string[]; slot: string; cues: Set<string> };
export const THING_KINDS: readonly string[], THING_CLASSES: readonly ThingClass[], THING_WEARS: readonly string[], THING_AURAS: readonly string[], THING_PARTS: readonly string[];
/** The character-creation picture (`creation`, ANSI only): a race beside a measuring stick, or a class with its tools. Colours are VGA indexes (0 to 15). */
export interface RaceFacts { key: string; inches: number; bulk: number; darkvision: boolean; skin: number; hair?: number; clothes: number; feature: string; featureColor?: number; known: boolean }
export interface ClassFacts { key: string; kind: string; clothes: number; trim: number; known: boolean }
/** What a race is drawn with: its row in the table, else made from its name by Coupler's seed and generator (the same every time). */
export function raceFacts(name: string): RaceFacts;
/** What a class is drawn with: its row, else made from its name. */
export function classFacts(name: string): ClassFacts;
/** Coupler's seed for a name: FNV-1a 64 of the kind ("race" or "class"), a zero byte, the key (letters only, lower case) and four zero bytes. */
export function portraitSeed(kind: 'race' | 'class', key: string): bigint;
/** Coupler's generator (xorshift64*), for the fallbacks to match; new art uses `rng`. */
export function couplerRng(seed: bigint | string): { next(): bigint; below(n: number): number; between(a: number, b: number): number; chance(percent: number): boolean };
/** Bulk in tenths of a human's width from weight per inch, 6 to 15. */
export function bulkFor(inches: number, pounds: number): number;
export const CREATION_KINDS: readonly string[], CLASS_KINDS: readonly string[], RACE_FEATURES: readonly string[], RACE_NAMES: readonly string[], CLASS_NAMES: readonly string[];
/** The figure layer (`recipes/figure.js`): a canvas of VGA colours whose pixels carry a part (0 back, 1 body, 2 hair, 3 eye, 4 mouth, 5 worn, 6 brow, 7 detail) and a lit flag. */
export class Layer { constructor(w: number, h: number, depth?: Colors); w: number; h: number; depth: Colors; a: Uint32Array; f: Uint8Array; parts: Uint8Array; outdoors: boolean; night: boolean; clone(): Layer }
/** How many colours a picture is drawn in: 16 (Bayer dithers), 256 (the colour between, from the xterm-256) or `'true'` (the exact colour between, in the cells' `tc`). 16 is the default everywhere and its output does not change. */
export type Colors = 16 | 256 | 'true';
export interface FigurePixels { w: number; h: number; colors: Uint8Array | Uint32Array; depth?: Colors; parts: Uint8Array; lit?: Uint8Array; partNames?: readonly string[] }
export interface PlaceOptions { colors?: Colors; terrain?: string; time?: string; weather?: string; style?: 'fantasy' | 'high'; key?: string; arc?: number; recede?: boolean; wild?: boolean; offset?: { x: number; y: number }; invisible?: boolean }
/** An animal's place: an outdoors terrain stays unless city, spaceport or seaport; else woods, plains or hills by its key. */
export function wildTerrain(terrain: string, key?: string): string;
/** The picture of a place at a pixel size (`columns` by `rows * 2`), a step darker unless `recede: false`; `avoid` is the columns of a head. */
export function placeLayer(opts: PlaceOptions & { columns: number; rows: number; avoid?: [number, number] }): Layer;
export function lay(layer: Layer, figure: FigurePixels, opts?: { offset?: { x: number; y: number }; hide?: boolean }): Layer;
export function stamp(layer: Layer, figure: FigurePixels, x0: number, y0?: number, opts?: { mirror?: boolean; flash?: boolean }): Layer;
export function lightFigures(layer: Layer, opts?: { outdoors?: boolean; night?: boolean }): Layer;
export function headSpan(figure: FigurePixels): [number, number] | undefined;
/** `colors` is the figure's own depth by default. */
export function figureOnPlace(figure: FigurePixels, place?: PlaceOptions, opts?: { bgLimit?: BgLimit; colors?: Colors }): { cells: Cells; layer: Layer; terrain: string };
export function darken(color: number, x: number, y: number): number;
export const LIGHTER: readonly number[], SHADE: readonly number[], RECEDE: readonly number[], SUN_ARCS: readonly number[], CLEAR: 255;
export interface FightFrame { you: { shift: number; flash: boolean }; foe: { shift: number; flash: boolean } }
/** The frames of a blow on `hit` ('you' or 'foe'); reduced motion gives a flash alone, or none. */
export function blowFrames(hit: 'you' | 'foe', flash?: boolean, reduced?: boolean): FightFrame[];
export function slideFrames(width: number, reduced?: boolean): FightFrame[];
export const FIGHT_LAYOUTS: readonly string[], BIG_HIT: 15, BLOW_MS: 100, SLIDE_MS: 24, FLASH_GAP_MS: 334;
export interface LightSource { x: number; y: number; radius: number; /** In ramp steps; default 2. */ strength?: number }
/** Light a picture: `ambient` steps everywhere (negative for night), `sources` falling off to nothing at their radius (a cell counts twice as tall as wide), `selfLit` cells (a `Cells` mask or a predicate) taking no ambient. New cells, legal for `bgLimit`. */
export function light(cells: Cells, opts?: { ambient?: number; sources?: LightSource[]; selfLit?: Cells | ((x: number, y: number, cell: Cell) => boolean); bgLimit?: BgLimit }): Cells;
export interface AnsimationOutput {
  /** The animation: a full first frame, then cursor-addressed deltas, each opened by a bare `ESC[H`. */
  bytes: Uint8Array;
  /** The last frame as an ordinary `.ans` (same options): the picture for reduced motion and for hosts that cannot play it. */
  still: Uint8Array;
  /** Bytes and seconds each frame takes to send at `baud`. */
  frames: { bytes: number; seconds: number }[];
  seconds: number; baud: number; charset: 'cp437' | 'utf8'; target: AnsiTarget; colors: 16 | 256 | 'true'; width: number; height: number; warnings: string[];
}
/** Frames of cells (one size) to an ANSImation. `baud` (default 2400) and `fps` (default 8) only decide the timing report and its warning. `sauce` writes SAUCE with the ANSiMation file type. */
export function writeAnsimation(frames: Cells[], opts?: { target?: AnsiTarget; charset?: 'cp437' | 'utf8'; colors?: 16 | 256 | 'true'; sauce?: boolean | SauceOptions; baud?: number; fps?: number }): AnsimationOutput;
/** Replay an ANSImation (or a plain `.ans`, as one frame): the whole screen at each frame mark. Without SAUCE the height is the last row drawn. */
export function readAnsimation(input: Uint8Array | string, opts?: { width?: number; charset?: 'auto' | 'cp437' | 'utf8'; ice?: boolean }): { frames: Cells[]; still: Cells; width: number; height: number; sauce: Sauce | null; charset: 'cp437' | 'utf8'; ice: boolean; warnings: string[] };
export interface Span { text: string; fg: number | null; bg: number | null; blink: boolean }
export function toSpans(cells: Cells): { columns: number; rows: number; lines: Span[][] };
/** The characters alone, one string a row joined by `\n`, trailing spaces cut. */
export function toPlainText(cells: Cells): string;

export const VGA16: readonly string[];
export const V: Readonly<Record<'black' | 'red' | 'green' | 'brown' | 'blue' | 'magenta' | 'cyan' | 'gray' | 'darkgray' | 'brightred' | 'brightgreen' | 'yellow' | 'brightblue' | 'brightmagenta' | 'brightcyan' | 'white', number>>;
export const XTERM256: readonly string[];
/** DB32 index to VGA16 index, by hand. */
export const DB32_VGA: readonly number[];
/** Hand-made shade ramps (VGA indexes, dark to light), each step pair buildable with `ladder` at 8 backgrounds. */
export const ANSI_RAMPS: Readonly<Record<'gray' | 'blue' | 'fire' | 'green' | 'magenta', readonly number[]>>;
export function nearest(hex: string, depth?: 16 | 256): { index: number };
export function nearest(hex: string, depth: 'true'): { hex: string };
export function indexHex(index: number): string;
export interface Recipe {
  describe: string; variants(): Record<string, any>[];
  /** The pixel drawing; absent when `ansiOnly`. */
  draw?(params?: Record<string, any>): Art;
  /** True for a recipe with no pixel drawing (banner, landscape, menu, bust, artpack, weatherloop, vista): draw it with style `ansi`. */
  ansiOnly?: boolean;
  /** A native ANSI drawing, preferred over folding the pixel one. */
  ansi?(params?: Record<string, any>): Art<Cells>;
  /** A native plain-text drawing, preferred over reading the ANSI or pixel one. */
  text?(params?: Record<string, any>): Art<Cells>;
}
export const RECIPES: Record<string, Recipe>;
export const RECIPE_IDS: string[];
export const STYLES: Style[];
export const DEFAULT_STYLE: 'pixel';
/** The styles a recipe can draw in: both (a pixel drawing folds to cells), or `ansi` alone for an `ansiOnly` recipe. */
export function recipeStyles(id: string): Style[];
/** Whether a recipe draws a style natively (`pixel` for every recipe but an `ansiOnly` one) rather than by folding. */
export function isNative(id: string, style: Style): boolean;
/** A pose for the `motion` param of any recipe: the channels of its stances (integers; y is up-negative; `legs` are the two feet's offsets). `air` is ignored. */
export interface MotionPose { dx?: number; dy?: number; lean?: number; head?: number; squat?: number; lift?: number; drag?: number; sway?: number; legs?: [number, number]; arm?: string; weapon?: string; air?: number }
/** A clip of poses (Animator's clip is one): `draw(id, { ...params, motion })` adds it to the art's `clips` under `id`; `character` draws each pose with its joints, any other recipe moves its first frame (see `moveFrame`). `holds` repeats a frame; `events` frames index the unrepeated poses. */
export interface MotionClip { id?: string; frames: MotionPose[]; fps?: number; loop?: boolean; events?: { frame: number; name: string }[]; holds?: number[]; reduced?: MotionPose[]; /** carry what leaves one edge in at the other (`dx dy lift`), so a tile stays tiled */ wrap?: boolean }
/** One still (Pixels or Cells) moved into a pose: `dx dy lift` move it, `squat` squashes it from the bottom, `lean` shears the top, `sway` shears the lower half; the joint channels are ignored. */
export function moveFrame<F extends Pixels | Cells>(still: F, pose?: MotionPose, options?: { wrap?: boolean }): F;
/** An art with `motion` added to its `clips`, drawn from its first frame. `draw` does this for every recipe but `character`. */
export function withMotion<A extends Art<any>>(art: A, motion: MotionClip | MotionClip[]): A;
/** A motion checked and given ids and holds; throws on a bad clip or pose channel. */
export function motionClips(motion?: MotionClip | MotionClip[]): (Required<Pick<MotionClip, 'id' | 'frames' | 'holds' | 'events' | 'fps' | 'loop'>> & { reduced?: MotionPose[] })[];
export const POSE_KEYS: string[];
/** A preset of a motion type: its own params with defaults (`null` = worked out from the art's size) and a maker of the clip. */
export interface MotionPreset { describe: string; params: Record<string, number | string | null>; make(size: { w: number; h: number }, params: Record<string, any>): Omit<MotionClip, 'id'> }
/** A kind of movement shared by some recipes. `channels`: a share of the art's width (`dx lean sway`) or height (`dy lift squat`) a channel may move by, or `true` for a joint channel. */
export interface MotionType { describe: string; channels: Record<string, number | true>; presets: Record<string, MotionPreset> }
/** The `motion` of a recipe may also name a preset of its type (`'bob'`), or give `{ preset, id?, ...params }`; a list may mix them with clips. */
export type MotionSpec = MotionClip | string | { preset: string; id?: string; [param: string]: unknown };
export const MOTION_TYPES: Record<'figure' | 'head' | 'creature' | 'object' | 'limb' | 'plant' | 'machine' | 'structure' | 'tiling' | 'scenery' | 'cloth' | 'effect' | 'panel' | 'lettering', MotionType>;
/** The motion type id of every recipe. */
export const MOTION_OF: Record<string, string>;
/** A recipe's motion type with its id. Throws on a recipe with none. */
export function motionType(recipe: string): MotionType & { id: string };
/** The names of a recipe's motion presets. */
export function motionPresets(recipe: string): string[];
/** Presets turned into clips for an art of `size`; clips pass through. Throws on an unknown preset or param. */
export function resolveMotion(recipe: string, motion: MotionSpec | MotionSpec[] | undefined, size: { w: number; h: number }): MotionClip[];
/** What a motion asks past its recipe's type: a channel the type does not use (`channel`) or a move beyond its limit (`range`). Never throws on those. */
export function lintMotion(recipe: string, motion: MotionSpec | MotionSpec[], size: { w: number; h: number }): { rule: 'channel' | 'range'; clip: string; frame: number; channel: string; message: string }[];
/** Draw by recipe id (item, face, creature, character, scene, prop, tile, hand, cloth, furniture, room, effect). Same params and seed, same pixels.
 *  `options.style` defaults to `pixel`; `ansi` gives Cells for frames (the recipe's native drawing, else the pixel one folded); `text` gives ASCII Cells (a native `text` drawing, else the ANSI one read as ink, else the pixel one read as light and dark). */
/** `options.atlas` is the VGA atlas, needed only by `overlay` with a stack `count`. */
export function draw(id: string, params?: Record<string, any>, options?: { style?: 'pixel'; atlas?: GlyphAtlas }): Art;
export function draw(id: string, params: Record<string, any> | undefined, options: { style: 'ansi' | 'text'; bgLimit?: BgLimit }): Art<Cells>;
export function draw(id: string, params?: Record<string, any>, options?: { style?: Style; bgLimit?: BgLimit }): Art | Art<Cells>;

export function hashSeed(seed: unknown): number;
export interface Rng { next(): number; int(n: number): number; range(a: number, b: number): number; chance(p: number): boolean; pick<T>(a: T[]): T; fork(label: string): Rng }
export function rng(seed: unknown): Rng;

export const DB32: Palette;
/** Named colour values: C.night, C.red, ... */
export const C: Readonly<Record<string, Color>>;
/** Four-step ramps, dark to light. */
export const RAMPS: Readonly<Record<string, readonly [Color, Color, Color, Color]>>;
export function ramp(name: string): readonly [Color, Color, Color, Color];
export function luma(hex: string): number;

export function shade(mask: Pixels, parts: Record<number, readonly Color[]>, opts?: { outline?: Color | null }): Pixels;
export function rim(mask: Pixels, from: Color, to: Color): Pixels;
export function dither(mask: Pixels, c: Color, phase?: number): Pixels;

export function squash(px: Pixels, factor: number): Pixels;
export function flash(px: Pixels, c?: Color): Pixels;
export function idle(px: Pixels, soft?: boolean): Pixels[];
export function collapse(px: Pixels, n?: number): Pixels[];
export function clip(id: string, frames: Pixels[], opts?: { fps?: number; loop?: boolean; events?: any[]; reduced?: Pixels[] }): Clip;

export interface Finding { level: 'error' | 'warn'; rule: string; message: string }
export const SIZES: number[];
export function lint(p: Pixels, opts?: { maxColors?: number; name?: string; skip?: string[] }): Finding[];
export interface AnsiLintOptions { target?: AnsiTarget; width?: number; maxHeight?: number; depth?: 16 | 256 | 'true'; name?: string; skip?: string[] }
/** The ANSI rules: width, height, colours, bg-limit, blink, charset, invisible, contrast, shade-noise, ramp, outline. (`gap` is not written yet.) */
export function lintAnsi(cells: Cells, opts?: AnsiLintOptions): Finding[];
/** An animation of cells: identical frames, a long loop, and flashing (more than three a second is an error). */
export function lintAnsiFrames(frames: Cells[], opts?: { name?: string; fps?: number; loop?: boolean }): Finding[];
/** What a compositor and a screen reader need: `alt`, anchors inside the canvas, a `reduced` still for the art and each clip that moves. All errors. */
/** Do these drawings differ by silhouette alone? Each pair, as filled shapes, must differ by at least `floor` (default 0.15) of their union; the class check for the top-down set. */
export function lintSilhouettes(items: { name: string; px: Pixels }[], opts?: { floor?: number }): { rule: 'silhouette'; level: 'error'; message: string }[];
export function lintArt(art: Art, opts?: { name?: string }): { rule: 'alt' | 'anchors' | 'reduced'; level: 'error'; message: string }[];
export function lintFrames(frames: Pixels[], opts?: { name?: string; loop?: boolean; boil?: boolean }): Finding[];
export function compare(a: Pixels, b: Pixels): Record<string, number>;

export interface KitSubject { id: string; label: string; recipes?: { recipe: string; params?: Record<string, any> }[]; provides?: string; partial?: string; reference?: string }
export interface Kit { id: string; title: string; doc: string; subjects: KitSubject[] }
export const KITS: Kit[];
export function registerKit(kit: Kit): void;
export function subjectStatus(subject: KitSubject): { status: 'covered' | 'partial' | 'gap' | 'built-in' | 'reference-only'; problems: string[]; warnings: string[] };
export function coverage(kits?: Kit[]): { id: string; title: string; doc: string; rows: any[]; totals: Record<string, number> }[];

export interface Look<S = any> { key: string; spec: S; art: Art; still: any; clip(name: string): any[] | null; fps(name: string): number }
/** Draw once per distinct spec and keep the latest few. `toUrl` turns one frame into whatever the host shows. */
export function createLookCache<S = any>(draw: (spec: S) => Art, toUrl: (px: Pixels) => any, opts?: { keep?: number }): { get(spec: S): Look<S>; clear(): void; readonly size: number };

export function toImageData(px: Pixels, scale?: number, bg?: string | null): ImageData;
export function toCanvas(px: Pixels, opts?: { scale?: number; bg?: string | null; canvas?: HTMLCanvasElement }): HTMLCanvasElement;
export function toDataUrl(px: Pixels, opts?: { scale?: number; bg?: string | null; canvas?: HTMLCanvasElement }): string;
export function drawFrame(ctx: CanvasRenderingContext2D, px: Pixels, x?: number, y?: number, scale?: number): void;
/** Play a clip on a canvas; returns `stop`. */
export function play(canvas: HTMLCanvasElement, clip: { frames: Pixels[]; fps?: number; loop?: boolean }, opts?: { scale?: number; loop?: boolean; onFrame?(i: number): void; onEnd?(): void; raf?(f: () => void): void; now?(): number }): () => void;

/** Steps in the artists' order whatever order they are written in: focal, outline, base, highlights, shadows, blend. */
export interface PaintContext {
  cells: Cells; focal: any;
  /** Draw a cell and protect it, so later steps keep it. */
  outline(x: number, y: number, cell: Partial<Cell>): void;
  /** Draw a cell unless it is protected. */
  put(x: number, y: number, cell: Partial<Cell>): void;
  isProtected(x: number, y: number): boolean;
  /** Every unprotected cell set to `cell`. */
  fill(cell: Partial<Cell>): void;
  /** Another drawing's non-empty cells as protected outline. */
  stamp(src: Cells, dx?: number, dy?: number): void;
}
export type PaintSteps = Partial<Record<'focal' | 'outline' | 'base' | 'highlights' | 'shadows' | 'blend', (ctx: PaintContext) => any>>;
export function paint(cells: Cells, steps: PaintSteps): Cells;
/** The same order for a drawing that is not on Cells; `ctx.focal` holds what `focal` returned. */
export function runSteps<C extends Record<string, any>>(steps: Partial<Record<'focal' | 'outline' | 'base' | 'highlights' | 'shadows' | 'blend', (ctx: C) => any>>, ctx?: C): C;
export const OVERLAY_IDS: string[];
/** Scatter stars, rain, snow, waves or runes (CP437 glyphs) by seed over cells that show no character yet. */
export function overlay(cells: Cells, kind: 'stars' | 'rain' | 'snow' | 'waves' | 'runes', seed: unknown, opts?: { region?: { x: number; y: number; w: number; h: number }; density?: number; fg?: number; where?(x: number, y: number): boolean }): Cells;
export const LETTERING_FACES: string[];
export const LETTERING_CHARS: string;
/** Text as block lettering (3 by 5 font) in a face: half, block, outlined or shaded. */
export function lettering(text: string, opts?: { face?: 'half' | 'block' | 'outlined' | 'shaded'; fg?: number; edge?: number; maxWidth?: number }): Cells;

// --- Tag Team phase 1: the contract with a compositor (docs/tag-team-gaps-plan.md) ---
/** The engine's version. Packs and `resolveLook` carry it; a test keeps it equal to package.json. */
export const ENGINE_VERSION: string;
/** A copy of `px` with each pixel of ramp `from` drawn in the same step of ramp `to` (ramps by name or four colour values). Colours two ramps share are ambiguous in a finished picture: the first step wins. */
export function recolor(px: Pixels, from: string | readonly Color[], to: string | readonly Color[]): Pixels;
/** Recolour a whole drawing (frames, layers, clips) by `[from, to]` pairs, each applied to the original pixels. */
export function recolorArt<A extends Art>(art: A, pairs: [string | readonly Color[], string | readonly Color[]][]): A;
/** Every option explicit (defaults written in), JSON-safe, `motion` left out. `explicit` is true for character, creature and item; other recipes come back as given. */
export function resolveLook(recipe: string, params?: Record<string, any>): { recipe: string; params: Record<string, any>; engine: string; explicit: boolean };
/** Draw with colour groups set: `dye('character', { loadout: 'rogue' }, { cloth: 'blue' })`. Draws again, so shading and outline are exact. */
export function dye(recipe: 'character' | 'creature' | string, params?: Record<string, any>, groups?: Record<string, string>, options?: { style?: 'pixel' }): Art;
/** The dye groups of each recipe that has them, and the param each sets. */
export const DYE_GROUPS: Record<string, Record<string, string>>;
/** Anchors from the box a drawing fills: `root`, `shadow_center`, `head`, `effect_origin`. */
export function boxAnchors(px: Pixels): Record<'root' | 'shadow_center' | 'head' | 'effect_origin', [number, number]>;
/** The short phrase `draw()` puts in `alt`. */
export function altOf(id: string, params: Record<string, any>, art: Art): string;
/** The projection a recipe draws in, where it is known. */
export const PROJECTION: Record<string, string>;
/** The `bodyProfile` values of `character`, as `[x scale, y scale]` about the feet, inside the unchanged 32 by 32 canvas. */
/** The four facings of `view: 'top'` (`west` is `east` mirrored). */
export const FACINGS: readonly ['north', 'east', 'south', 'west'];
export const BODY_PROFILES: Record<'medium' | 'small' | 'slim' | 'broad', [number, number]>;
export const PLACEHOLDER_KINDS: readonly string[], PLACEHOLDER_SIZES: readonly string[];
export const PLACEHOLDER_FAMILIES: Record<string, string>, PLACEHOLDER_CATEGORIES: Record<string, string>;

/** The IBM VGA 8x16 atlas as decoded RGBA (`loadAtlas` from `art-sprites/atlas`). */
export interface GlyphAtlas { w: number; h: number; rgba: Uint8Array }
/** `text` in the VGA font as a canvas, 8 pixels a character by 16 high (a pixel border of `outline` when set). `ink` and `outline` are palette values (`C.white`). */
export function pixelText(atlas: GlyphAtlas, text: string, opts?: { ink?: number; outline?: number | null }): Pixels;
/** A floating combat number in the VGA font: a 4-frame rise (`clips.float`). `kind` damage, crit, heal, miss or block. */
export function drawNumber(atlas: GlyphAtlas, opts?: { value?: number; kind?: 'damage' | 'crit' | 'heal' | 'miss' | 'block' }): Art;
export const NUMBER_KINDS: Record<string, { ink: string; outline: string; prefix?: string; word?: string }>;
export const TELEGRAPH_SHAPES: readonly ('circle' | 'line' | 'cone' | 'rect')[];
export const TARGET_KINDS: readonly ('ally' | 'enemy' | 'neutral' | 'turn' | 'seat' | 'shadow')[];
/** The rarity frames (`common` to `legendary`) and the state markers `overlay` draws. */
export const RARITIES: Record<'common' | 'uncommon' | 'rare' | 'epic' | 'legendary', string>;
export const MARKS: Record<string, { at: 'tl' | 'tr' | 'bl' | 'br'; ink: string; accent: string; rows: string[] }>;

/** Enemy families (T13): the parts a creature of the family may have. `creature({ family, seed })` composes one by seed and refuses a part the family does not allow. */
export const FAMILIES: Record<string, { bodies: string[]; skin: string[]; eyes: string[]; jaw: string[]; horns: number[]; ears: boolean[]; gear?: string[][] }>;
export const FAMILY_IDS: readonly string[];
/** The creature options of `family` composed by `opts.seed`, with what `opts` names on top. Throws on a part the family does not allow. */
export function composeFamily(family: string, opts?: Record<string, any>): Record<string, any>;
/** The clips every creature has beyond idle, hit and defeat (T14), each with events and a reduced still. */
export const ENEMY_CLIPS: readonly ('telegraph' | 'attack' | 'attack_ranged' | 'attack_magic' | 'stagger' | 'block' | 'enrage' | 'summon' | 'retreat' | 'dissolve' | 'phase')[];
/** `statuses` of `creature`: drawn as a transparent layer in `layers.status`. `intent` is a marker in `layers.intent`. `rank` is the elite and boss treatment. */
export const STATUSES: readonly ('poison' | 'burn' | 'freeze' | 'stun' | 'bleed')[];
export const INTENTS: readonly ('attack' | 'defend' | 'magic' | 'flee' | 'summon')[];
export const RANKS: readonly ('normal' | 'elite' | 'boss')[];
/** The plan each creature body is drawn as from above (`view: 'top'`), and the canvases it can have (a boss draws again at 48, 64, 96, 128, 192 or 256, it is not enlarged). */
export const TOP_PLAN: Record<string, string>;
export const TOP_CANVASES: readonly [40, 48, 64, 96, 128, 192, 256];
/** The ids of a boss's `parts` by the number of `segments` (2 to 5); a part's `joint` is the cut it hangs from, and the parts lay back into the still exactly. */
export const SEGMENT_IDS: Record<2 | 3 | 4 | 5, readonly string[]>;
/** `tile` with `kind: 'dungeon'` and `view: 'top'`: the parts, biomes and sizes of the top-down set. `mask` bits are north 1, east 2, south 4, west 8; `notch` bits north east 1, south east 2, south west 4, north west 8. */
export const TOP_PARTS: readonly string[], TOP_BIOMES: readonly string[], TOP_SIZES: readonly [16, 32], DOOR_STATES: readonly ('open' | 'closed' | 'locked')[];
export const TOP_LOOKS: Record<string, { floor: string; wall: string; style: string; glow: string | null; inlay?: boolean }>;
export const NOTCHES: readonly [number, number][];
/** `prop` with `view: 'top'`: the ids. Each art carries `footprint` (tiles), `occlusion` (`none`, `partial`, `blocks-sight`), `states` and clips (`reveal` on all; `open`, `trigger`, `flicker`, `glow`, `swirl` where they apply). */
export const TOP_PROP_IDS: readonly string[];
/** `dress`: the props each template scatters, and `lintDress(art)`, the cells whose picture does not match the grid (empty when the art implies no path the grid lacks). */
export const TEMPLATES: Record<string, string[]>, TEMPLATE_IDS: readonly string[];
export function lintDress(art: Art): [number, number][];
/** `map`: the states the game reports for a cell, the terrain kinds, the markers and the two sizes (pixels a cell). */
export const SPLASH_SCENES: readonly ('meadow' | 'forest' | 'desert' | 'mountains' | 'sea' | 'snow' | 'night')[], SPLASH_W: 256, SPLASH_H: 144, SPLASH_MAX_TITLE: 28;
export const MAP_STATES: readonly ('unknown' | 'visible' | 'remembered' | 'explored')[], MAP_KINDS: readonly string[], MAP_MARKERS: readonly string[];
export const MAP_SIZES: Record<'mini' | 'expanded', number>;

/** Phase 4 (T8): every semantic clip id (the core six and the expanded list), and which attack clip a weapon shape takes (`attack`, `shoot`, `thrust`, `slam` or `cast`). */
export const SEMANTIC_IDS: readonly ('turn' | 'block' | 'cast' | 'heal' | 'celebrate' | 'dodge' | 'parry' | 'charge' | 'stun' | 'potion' | 'buffed' | 'lowhealth' | 'levelup' | 'tagin' | 'tagout' | 'equip' | 'shoot' | 'thrust' | 'slam' | 'recall')[];
export function weaponAttack(shape: string): 'attack' | 'shoot' | 'thrust' | 'slam' | 'cast';
/** Phase 4 (T7): the options of the extra `character` layers `neck`, `scar`, `jewellery` and `helmStyle` (`tabard`, `bracers` and `cloak` take a ramp name; the cloak is a layer apart from the `back` slot, so a pack and a cloak can be worn together). */
export const NECK_KINDS: readonly ('amulet' | 'scarf' | 'collar')[], SCARS: readonly ('brow' | 'cheek' | 'mouth')[], JEWELLERY: readonly ('earring' | 'brooch')[], HELM_STYLES: readonly ('visored' | 'crested' | 'plumed' | 'winged')[];
/** Phase 4 (T10): the `material` kinds, the coin denominations (ramps) and the `currency` kinds and key looks. */
export const MATERIAL_IDS: readonly ('ore' | 'herb' | 'hide' | 'part' | 'crystal')[], DENOMINATIONS: Record<'copper' | 'silver' | 'gold' | 'platinum', readonly number[]>;
export const CURRENCY_KINDS: readonly ('coin' | 'purse' | 'key' | 'fragment' | 'plan' | 'token')[], KEY_LOOKS: readonly ('bronze' | 'silver' | 'gold' | 'skeleton')[];
/** Phase 4 (T20): the building grammar. A `service` gives a default `shell`, `theme` and icon `glyph`; `building` and `town` use them. `town` returns `hotspots` (`{ id, service, state, rect, entry }`), a `grid` of 32 pixel cells (`#` building or tree, `.` street, `,` grass) and `cell` 32. */
export const SERVICES: Record<string, { shell: string; glyph: string; theme: string }>, SERVICE_IDS: readonly string[], SHELLS: readonly ('cottage' | 'hall' | 'tower' | 'vault' | 'stall' | 'wing')[];
/** Phase 7: how a `town`'s streets run (`paths`). A `building` with a state marker takes `animate: true` for a four-frame loop. */
export const TOWN_PATHS: readonly ('lattice' | 'rows' | 'cross')[];
export const THEMES: Record<'village' | 'stone' | 'arcane' | 'market', { roof: string; walls: string }>, BUILDING_STATES: readonly ('locked' | 'available' | 'new' | 'upgrade')[];
/** Phase 6: the canvases an `item` icon can be drawn on (`iconSize`; 24 and 32 are drawn again from the shape, not scaled). */
export const ICON_SIZES: readonly [16, 24, 32];

/** Phase 4 (T21): `exhibit` styles and states (a `picture` is any art, Pixels or `{ recipe, params }`; the art carries `window`, where the picture sits) and `award` kinds and tiers. */
export const EXHIBIT_STYLES: readonly ('case' | 'plinth' | 'frame' | 'page' | 'board')[], EXHIBIT_STATES: readonly ('empty' | 'occupied' | 'new')[];
export const AWARD_KINDS: readonly ('plaque' | 'podium' | 'ribbon' | 'seal' | 'medal' | 'rank')[], AWARD_TIERS: Record<'bronze' | 'silver' | 'gold' | 'platinum', readonly number[]>;
