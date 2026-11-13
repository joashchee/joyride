# Formats Neumetik doesn't play

These formats were planned (`tracker-formats.md`, section 3) and dropped.
Neumetik's rule is that every replayer and chip is written from
licence-clear public material: format descriptions, manuals, datasheets,
or source under MIT, BSD, zlib, ISC, the Unlicense or the public domain.
None of these has any. Their only replayers are rips or disassemblies of
commercial programs, source published with no licence, GPL code, or a
program still sold. A file of one, which no format reads, says which it
is, why it isn't played and what plays it (`src/formats/dropped.rs`, the
code copy of the table below; a test keeps the two in step).

**Researched: 2026-10-15.** File counts are Modland's on that day. A
format comes back if its author grants a licence, or a description turns
up: see "Could come back" below.

## The list

| Format | Files | Known by | Why it's dropped | What plays it |
|---|---|---|---|---|
| Future Player | 9 | `FP.` prefix; executables saying `F.PLAYER` | Paul van der Valk's replayer (Hybris, Imploder), known only through Wanted Team's resourced rip, whose terms say not to assume it's open source | [UADE](https://gitlab.com/uade-music-player/uade) (`FuturePlayer`), [foobar2000's UADE Input](https://www.foobar2000.org/components/view/foo_input_uade) on Windows, [EaglePlayer](https://aminet.net/package/mus/play/Eagleplayer) with Wanted Team's player on an Amiga |
| Synth Dream | 22 | `SDR.` (samples `SMP.`) | Only Wanted Team's unlicensed source | UADE (`SynthDream`), EaglePlayer |
| Synth Pack | 17 | `OSP.` | Only a Wanted Team binary | UADE (`SynthPack`), EaglePlayer, [HippoPlayer](https://github.com/koobo/HippoPlayer) |
| Dynamic Synthesizer | 5 | `DNS.` (samples `SMP.`) | Only a Wanted Team binary (Chris Huelsbeck's format before TFMX) | UADE (`DynamicSynthesizer`), EaglePlayer |
| Beathoven Synthesizer | 32 | `BSS.` | Each song carries its game's own ripped 68000 replayer | UADE (`BeathovenSynthesizer`), EaglePlayer, HippoPlayer |
| Mark II | 35 | `MK2.`, `MKII.` | Only binaries adapted by EaglePlayer's and DeliTracker's authors | UADE (`MarkII`), [DeliTracker 2.32](https://aminet.net/package/mus/play/DeliTracker232), EaglePlayer |
| Medley | 4 | `MSO.` | The author's own replayer is in UADE, but its [terms](https://gitlab.com/uade-music-player/uade/-/blob/master/amigasrc/players/uade/medley/COPYING) keep his copyright and bar reuse in another program | UADE (`Medley`), DeliTracker 2.32 |
| The Musical Enlightenment | 15 | `TME.` | Only Wanted Team's unlicensed source and a DeliTracker binary | UADE (`TheMusicalEnlightenment`), DeliTracker, EaglePlayer |
| Sound Programming Language | 9 | `SPL.` | Holger Gehrmann's SOPROL (Magic Bytes and Rainbow Arts games): only Wanted Team's player of the games' own ripped code | UADE (`SoundProgrammingLanguage`), EaglePlayer |
| Quartet | 564 | `.4v` (+ `.set`), `.sqt` | The Atari ST's Quartet: nothing describes the format; Quartet PSG files are 68000 code | UADE (`Quartet_ST`, `Quartet_PSG`), EaglePlayer |
| Tunefish 2 | 4 | `.tf2m` | Its synth's source was never released (Tunefish 4's [source](https://github.com/paynebc/tunefish) is GPL-3 and a different synth) | The [Tunefish 2](https://tunefish-synth.com/download) instrument in a VST host on Windows |
| JayTrax | 35 | `.jxs` | Reinier van Vliet's replayer source was published with no licence (Cog's copy was licensed GPL-3 by its porter) | [rePlayer](https://github.com/arnaud-neny/rePlayer) on Windows, [Cog](https://github.com/losnoco/Cog) on macOS |
| Ixalance | 24 | `.ixs` | The only source is a decompiled copy of the commercial IXSPlayer, licensed for non-commercial use | rePlayer, [webIXS](https://www.wothke.ch/webIXS/) in a browser |
| Buzzic | 4 | `.buz` | Released with no replayer source and no licence | [Buzzic](https://ftp.untergrund.net/users/stan_1901/Buzzic.zip) itself (Windows, .NET 1.1) |
| Buzzic 2 | 12 | `.buz2` | Its source was published with no licence | rePlayer |
| DarkWave Studio | 3 | `.dwp` | Its 2009 source was GPL-3; the program has been closed since | [DarkWave Studio](https://www.experimentalscene.com/software/darkwave-studio/) itself (Windows) |
| Mario Paint (Shroom) | 63 | `.sho` | Playing it needs Mario Paint's own sounds, which are Nintendo's | [Mariopants](https://battleofthebits.com/lyceum/View/Mariopants) (Windows, macOS, Linux) |
| Cybertracker (C64) | 32 | `.ct` | A closed editor and packer, and nothing describes the format | CyberTracker on a C64 or in VICE; its packer makes a `.sid` any SID player plays |
| SID Duzz' It | none | | No source, licence or description; its songs exist only as `.sid` rips | The rips in HVSC, DeepSID in a browser |
| NerdTracker II | 54 | `.ned` | The [source](https://github.com/et1999cc/Nerdtracker-II) has no licence and its replayer is for non-commercial use only; nothing describes the format | NerdTracker II itself, in DOSBox or Floppy |
| LSDJ | none | `.lsdsng` | The sound engine is the Game Boy program itself, which is sold | LSDJ, with your own copy, in a Game Boy emulator such as [SameBoy](https://github.com/LIJI32/SameBoy) |
| AY STRC | 1 | `.strc` | Nothing describes it, and no player is known | none known |
| AY Amadeus | 33 | `.amad` | Nothing describes it; its only player, Ay_Emul, is free to use but not to build on | [Ay_Emul](https://web.archive.org/web/2025/https://bulba.untergrund.net/emulator_e.htm) (Windows), imported by [Vortex Tracker II](https://ay.strangled.net/vortex_e.htm) |
| Beepola | 105 | `.bbsong` | Most songs play through beeper engines taken from commercial games and utilities | [Beepola](http://freestuff.grok.co.uk/beepola/help/) itself (Windows, or Wine) |
| TFM Music Maker | 77 | `.tfe` | Nothing describes it; the only open reader is ZXTune's (LGPL-3) | ZXTune, [foobar2000's ZXTune input](https://www.foobar2000.org/components/view/foo_input_zxtune) |
| MoonBlaster | 2,512 | `.mbm` (+ `.mbk`) | Its replayer is a copyrighted MSX program, and nothing describes the format | [MSXplay](https://msxplay.com/) (libkss), MoonBlaster on an MSX or in openMSX |
| SCC-Musixx | 97 | `.sng` (not used to recognize it: GoatTracker's too) | Nothing describes it, and nothing plays it today but the original | SCC-Musixx on an MSX or in openMSX |
| OPLLDRV | 373 | `.opx` | It plays only through its copyrighted MSX driver | MSXplay (libkss) |
| MuSICA | 808 | `.bgm` | It plays only through its copyrighted MSX driver, Kinrou5 | MSXplay (libkss) |
| MDX | 7,119 (+ 332 `.pdx`) | `.mdx` | Every player comes from a disassembly of the X68000's MXDRV, whose source was never released | [portable_mdx](https://github.com/yosshin4004/portable_mdx), foobar2000's MDX input, [mdxplay](https://m68k.aminet.net/package/mus/play/mdxplay_68k) on an Amiga |

## Original-era apps that play them (for Floppy)

Floppy runs these with the user's own copies (it never ships them):

| Guest | App | Program | Plays |
|---|---|---|---|
| Amiga | EaglePlayer 2.06 with Wanted Team's players (`EP_*` on Aminet) | `EaglePlayer` | Future Player, Synth Dream, Synth Pack, Dynamic Synthesizer, Beathoven Synthesizer, Mark II, The Musical Enlightenment, Sound Programming Language, Quartet: each its player's prefix |
| Amiga | DeliTracker 2.32 (freeware) | `DeliTracker` | Mark II, Medley, The Musical Enlightenment, MED |
| Amiga | HippoPlayer | `HiP` | Mark II, Beathoven Synthesizer, Medley, Synth Pack, Future Player, The Musical Enlightenment |
| Amiga | mdxplay 68k | `mdxplay` | MDX (with its `.pdx`) |
| DOS | NerdTracker II | its own program | NerdTracker II songs |

## Could come back

Each needs one thing. Until then they stay on this list:

- **JayTrax** and **Buzzic 2**: complete source exists; a licence from
  Reinier van Vliet, or from Stan/Rebels, would do.
- **Medley** and **Mark II**: their authors (Paul van der Valk; Darius
  Zendeh) could grant one.
- **MDX**: a licence-clear description of MXDRV's commands.
- **MoonBlaster**: a description of the format, or the replayer under a
  licence.
- **Quartet PSG**: playing each file's own 68000 code on a clean 68000
  and YM2149 is possible, but how the code is started isn't described.

The formats still planned, and the decisions some of them wait on, are
in `tracker-formats.md`, section 3.
