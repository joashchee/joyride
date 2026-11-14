# Sunscreen

A small, dependency-free virtual-resolution helper for Love2D. Copy `sunscreen.lua` into a project, draw in a fixed logical coordinate system, and Sunscreen handles resize scaling and pointer conversion.

## 🚀 Quick Start

Copy `sunscreen.lua` to your project and use:

```lua
local screen = require("sunscreen")

function love.load()
    screen:init({
        width = 800,       -- Logical game width
        height = 600,      -- Logical game height
        mode = "fit"       -- "fit", "fill", or "stretch"
    })
end

function love.draw()
    screen:push()
    -- Put your game-drawing code here
    screen:pop()
end

function love.touchpressed(id, x, y)
    local gameX, gameY = screen:toGame(x, y)
    -- Handle touch at game coordinates
end

function love.resize(w, h)
    screen:refresh(w, h)
end
```

## 📁 Files

- `sunscreen.lua` - The complete, standalone library (copy this to your project)
- `main.lua` - Interactive demo (Love2D game) showing all features

## Features

- **Universal**: Works on desktop, mobile, and any screen size/orientation
- **Single File**: Just copy `sunscreen.lua` - no dependencies
- **Virtual resolution**: Draw a fixed logical game area at any window size
- **Multiple Scaling Modes**: Fit, Fill, and Stretch options
- **Input conversion**: Convert mouse and touch positions in both directions
- **Explicit graphics state**: `push()` / `pop()` never modify Love2D callbacks

## API Reference

### Configuration
```lua
screen:init({ width = 800, height = 600, mode = "fit" })
```

### Core Functions
```lua
screen:push()            -- Apply scaling transformation
screen:pop()             -- Restore Love2D's previous transform
screen:toGame(x, y)      -- Convert screen coordinates to game coordinates
screen:toScreen(x, y)    -- Convert game coordinates to screen coordinates
screen:contains(x, y)    -- Whether a screen position is inside the game area
screen:refresh(w, h)     -- Recalculate after a resize; arguments are optional
screen:getGameWidth()    -- Get game width
screen:getGameHeight()   -- Get game height
screen:getMode()         -- Get current scaling mode
screen:setMode(mode)     -- Change scaling mode
screen:setGameSize(w, h) -- Change game dimensions
```

`gameWidth`, `gameHeight`, `apply`, `reset`, `screenToGame`, `updateScale`, and `onResize` remain as version 1.x compatibility aliases.

### Scaling Modes
- **`"fit"`** - Letterbox scaling (maintains aspect ratio, may show black bars)
- **`"fill"`** - Crop scaling (maintains aspect ratio, no black bars, may crop content)  
- **`"stretch"`** - Stretch scaling (fills entire screen, may distort content)

For `fit`, `contains(x, y)` returns `false` for letterbox bars. `fill` may crop the game area, while `stretch` uses independent X and Y scaling.

## License

MIT License - Feel free to use in your projects!