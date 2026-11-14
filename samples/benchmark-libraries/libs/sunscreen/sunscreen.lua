-- Sunscreen: virtual-resolution scaling for Love2D games.
-- Call screen:push() before drawing in virtual coordinates and screen:pop() after.
local screen = {
    _LICENSE = "This software is distributed under the MIT license. View the LICENSE on GitHub for details.",
    _URL = "https://github.com/challacade/sunscreen",
    _VERSION = "2.0.0",
    _DESCRIPTION = "Virtual-resolution scaling and input conversion for Love2D games",
}

local validModes = {
    fit = true,
    fill = true,
    stretch = true,
}

screen.width = 800
screen.height = 600
screen.mode = "fit"
screen.scale = 1
screen.scaleX = 1
screen.scaleY = 1
screen.offsetX = 0
screen.offsetY = 0
screen.screenWidth = 0
screen.screenHeight = 0

local transformActive = false

local function assertPositiveNumber(value, name)
    assert(type(value) == "number" and value > 0, name .. " must be a positive number")
end

local function getDimensions()
    assert(love and love.graphics, "Sunscreen requires love.graphics")
    return love.graphics.getDimensions()
end

function screen:init(options)
    options = options or {}
    local width = options.width or options.gameWidth or self.width
    local height = options.height or options.gameHeight or self.height
    local mode = options.mode or self.mode

    assertPositiveNumber(width, "width")
    assertPositiveNumber(height, "height")
    assert(validModes[mode], "mode must be 'fit', 'fill', or 'stretch'")

    self.width = width
    self.height = height
    self.mode = mode
    return self:refresh()
end

function screen:refresh(screenWidth, screenHeight)
    screenWidth = screenWidth or select(1, getDimensions())
    screenHeight = screenHeight or select(2, getDimensions())
    assertPositiveNumber(screenWidth, "screen width")
    assertPositiveNumber(screenHeight, "screen height")

    self.screenWidth = screenWidth
    self.screenHeight = screenHeight

    local widthScale = screenWidth / self.width
    local heightScale = screenHeight / self.height

    if self.mode == "stretch" then
        self.scaleX = widthScale
        self.scaleY = heightScale
        self.scale = math.min(widthScale, heightScale)
    else
        self.scale = self.mode == "fill"
            and math.max(widthScale, heightScale)
            or math.min(widthScale, heightScale)
        self.scaleX = self.scale
        self.scaleY = self.scale
    end

    self.offsetX = (screenWidth - self.width * self.scaleX) / 2
    self.offsetY = (screenHeight - self.height * self.scaleY) / 2
    return self
end

function screen:push()
    assert(not transformActive, "Sunscreen transform is already active; call screen:pop() first")
    self:refresh()
    love.graphics.push()
    love.graphics.translate(self.offsetX, self.offsetY)
    love.graphics.scale(self.scaleX, self.scaleY)
    transformActive = true
end

function screen:pop()
    if transformActive then
        love.graphics.pop()
        transformActive = false
    end
end

function screen:toGame(x, y)
    self:refresh()
    return (x - self.offsetX) / self.scaleX, (y - self.offsetY) / self.scaleY
end

function screen:toScreen(x, y)
    self:refresh()
    return x * self.scaleX + self.offsetX, y * self.scaleY + self.offsetY
end

function screen:contains(x, y)
    local gameX, gameY = self:toGame(x, y)
    return gameX >= 0 and gameX <= self.width
        and gameY >= 0 and gameY <= self.height
end

function screen:setMode(mode)
    assert(validModes[mode], "mode must be 'fit', 'fill', or 'stretch'")
    self.mode = mode
    return self:refresh()
end

function screen:setGameSize(width, height)
    assertPositiveNumber(width, "width")
    assertPositiveNumber(height, "height")
    self.width = width
    self.height = height
    return self:refresh()
end

function screen:getGameWidth() return self.width end
function screen:getGameHeight() return self.height end
function screen:getMode() return self.mode end
function screen:getVersion() return self._VERSION end

-- Compatibility names for version 1.x projects.
screen.apply = screen.push
screen.reset = screen.pop
screen.updateScale = screen.refresh
screen.onResize = screen.refresh
screen.screenToGame = screen.toGame

return screen