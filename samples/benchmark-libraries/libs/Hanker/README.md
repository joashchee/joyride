It's a UI lib! You can install it by copying the `hanker/` folder from this
repo into your game. everything else is just sample project stuff and tests.

Features:

* Mixed Immediate/Retained style: each UI control is defined ahead of time, and
  can be used to hold things like animation state: but the actual handling of
  UI events such as onClicked is done in immediate mode.
* Relative layout: complex UI layouts can be defined by declaratively placing
  controls relative to each other
* Resolution independence: Hanker lets you work in pixel-sized units, but will
  re-scale them automatically to match the current res.
* Customizable: Every kind of control can be skinned to match your game style,
  and custom controls can be registered with default skins.

This library natively supports BMFonts generated using Multi Signed Distance
Fields. The tool I used to generate this is packed in this repo under
[scripts/msdf-bmfont](scripts/msdf-bmfont). You can also use normal fonts, too~

![recording of circle-test.lua](https://gitlab.com/Alloyed/hanker/raw/master/circle-test.gif)

MIT license, (c) kyle mclamb.

## Docs

The most valuable one is probably [hello-world.lua](examples/hello-world.lua), which is a self-contained starting point for your own code.

For a comprehensive view of what Hanker supports, see the [API Documentation](docs/hanker.md).

You can also check out the annotated code samples in the [examples directory](examples).

## TODO
- [ ] Some equivalent to CSS classes
- [ ] More/better docs

### IMAGES
- [ ] There is no control that just shows an Image/Quad. Should there be?
- [ ] Built-in 9patch support? seems like it'd be handy

### CONTAINERS
- [ ] Make test case for containers

### EDITBOXES
- [ ] multi-line edit boxes

### FONTS
- [ ] automatically generate TTF font based on textSize
- [ ] Manually pick set of fonts to choose from based on textSize
- [ ] When a glyph isn't in a bitmap/MSDF font, fall back to TTF font/default font

### RENDERING
- [ ] Lots of drawcalls :/ some things can be autobatched by splitting into layers, but then controlling draw order automatically becomes dicey.
