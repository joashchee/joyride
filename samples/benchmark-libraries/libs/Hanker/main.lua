--[[
  Hanker test runner
  You should not include any of this in your app
--]]
love.filesystem.setRequirePath("?.lua;?/init.lua;lib/?.lua;lib/?/init.lua")
require 'imgui'
local Hanker = require 'hanker'

do
	local font = love.graphics.newFont("font.fnt")
	font:setFilter('linear', 'linear', 16)

	local hfont = Hanker.newMSDFFont(font, 24)
	hfont:setDefaultTextSize(12)

	Hanker.setDefaultFont(hfont)
end

--local editor = require 'control-editor'

local testFiles = {
	'tests/circular-menu.lua',
	'tests/fixed-grid.lua',
	'tests/label.lua',
	'tests/main-menu.lua',
	'tests/ps4-interface.lua',
	'tests/scrollList.lua',
	'tests/virtual-keyboard.lua',
}
local currentTestFile = testFiles[#testFiles]
local test = assert(love.filesystem.load(currentTestFile))()
local function testCombo()
	if imgui.BeginCombo("test", currentTestFile) then
		for _, testFile in ipairs(testFiles) do
			if imgui.Selectable(testFile) then
				test = assert(love.filesystem.load(testFile)) ()
				currentTestFile = testFile
			end
		end
		imgui.EndCombo()
	end
end

local prof
function love.load(argv)
	for _, arg in ipairs(argv) do
		if arg == '--profile' then
			_G.PROF_CAPTURE = true
			_G.PROF_REALTIME = false
		elseif arg == '--profile-realtime' then
			_G.PROF_CAPTURE = true
			_G.PROF_REALTIME = true
		else
			local testFile = arg
			test = assert(love.filesystem.load(testFile))()
			currentTestFile = testFile
		end
	end

	Hanker.setUnscaledDimensions(800, 600)
	Hanker.setViewport(0, 0, love.graphics.getDimensions())
	love.keyboard.setKeyRepeat(true)
end

local showEditor = false
function love.update(dt)
	--imgui
	prof.push("imgui.update")
	--imgui.NewFrame()
	if showEditor then
		--imgui.Text("FPS:"..love.timer.getFPS())
		testCombo()
		--editor:update()
	end
	prof.pop("imgui.update")

	-- ui
	prof.push("Hanker.updateFrame")
	Hanker.updateFrame(dt)
	--Hanker.updateMouse(love.mouse.getPosition())
	prof.pop("Hanker.updateFrame")

	prof.push("test.update")
	test.update(dt)
	prof.pop("test.update")
end

local keyFont = love.graphics.newFont(15)
local function drawKeys()
	love.graphics.push()
	local w, h = love.graphics.getDimensions()
	love.graphics.translate(w-40, h-40)
	local oldFont = love.graphics.getFont()
	love.graphics.setFont(keyFont)
	if love.keyboard.isDown('up') then
		love.graphics.print('^', 10, 0)
	end
	if love.keyboard.isDown('down') then
		love.graphics.print('v', 10, 15)
	end
	if love.keyboard.isDown('left') then
		love.graphics.print('<', 0, 15)
	end
	if love.keyboard.isDown('right') then
		love.graphics.print('>', 20, 15)
	end
	if love.keyboard.isDown('x') then
		love.graphics.print('x', 20, 0)
	end
	love.graphics.setFont(oldFont)
	love.graphics.pop()
end

function love.draw()
	prof.push("Hanker.draw")
	Hanker.draw()
	prof.pop("Hanker.draw")

	prof.push("test.draw")
	if test.draw then test.draw() end
	prof.pop("test.draw")

	love.graphics.setColor(1, 1, 1, 1)
	drawKeys()

	prof.push("imgui.Render")
	--imgui.Render()
	prof.pop("imgui.Render")
end

function love.resize(w, h)
	Hanker.setViewport(0, 0, w, h)
end

--
-- User inputs
--
function love.textinput(t)
	imgui.TextInput(t)
	if imgui.GetWantCaptureKeyboard() then
		return
	end

	Hanker.textInput(t)
end

local keymap = {
	x = "confirm",
	z = "cancel",
	c = "options",
	up = "up",
	down = "down",
	left = "left",
	right = "right",
	pageup = "pageup",
	pagedown = "pagedown",
	home = "first",
	["end"] = "last",
}

function love.keypressed(key)
	imgui.KeyPressed(key)
	if imgui.GetWantCaptureKeyboard() then
		return
	end

	if Hanker.keyboardKeyPressed(key) then
		return
	end

	-- input handling code.
	-- this is hardcoded here because I'm making the assumption that in a real
	-- game you may already have input handlers and a way to map keys to
	-- abstract actions.
	local cursorKey = keymap[key]
	if cursorKey then
		Hanker.cursorKeyPressed(cursorKey)
	elseif key == 'tab' then
		if love.keyboard.isDown('lshift', 'rshift') then
			Hanker.cursorKeyPressed('previous')
		else
			Hanker.cursorKeyPressed('next')
		end
	end
end

function love.keyreleased(key)
	imgui.KeyReleased(key)
	if imgui.GetWantCaptureKeyboard() then
		return
	end

	if key == 'e' and love.keyboard.isDown('lctrl', 'rctrl') then
		showEditor = not showEditor
		return
	end

	-- input handling code.
	-- this is hardcoded here because I'm making the assumption that in a real
	-- game you may already have input handlers and a way to map keys to
	-- abstract actions.
	local cursorKey = keymap[key]
	if cursorKey then
		Hanker.cursorKeyReleased(cursorKey)
	elseif key == 'tab' then
		Hanker.cursorKeyReleased('next')
		Hanker.cursorKeyReleased('previous')
	end
end

function love.mousemoved(x, y)
	imgui.MouseMoved(x, y)
	if not imgui.GetWantCaptureMouse() then
		Hanker.mouseMoved(x, y)
	end
end

function love.mousepressed(x, y, button)
	imgui.MousePressed(button)
	if not imgui.GetWantCaptureMouse() then
		-- Alt modifier means handle in imgui
		if not love.keyboard.isDown('lalt', 'ralt') then
			Hanker.mousePressed(x, y, button)
		end
	end
end

function love.mousereleased(x, y, button)
	imgui.MouseReleased(button)
	if not imgui.GetWantCaptureMouse() then
		Hanker.mouseReleased(x, y, button)
	end
end

function love.wheelmoved(x, y)
	imgui.WheelMoved(y)
	if not imgui.GetWantCaptureMouse() then
		Hanker.wheelMoved(x, y)
	end
end

function love.quit()
	if not PROF_REALTIME then
		--prof.popAll()
		prof.write("prof_capture.mpack")
	end
	imgui.ShutDown()
end

function love.run()
	if love.load then love.load(love.arg.parseGameArguments(arg), arg) end

	prof = require 'jprof'
	require 'hanker.profile-hooks' ()
	if PROF_REALTIME then
		prof.connect()
	else
		prof.enableThreadedWrite()
	end

	-- We don't want the first frame's dt to include time taken by love.load.
	if love.timer then love.timer.step() end

	local dt = 0

	-- Main loop time.
	return function()
		prof.push("frame")
		-- Process events.
		prof.push("events")
		if love.event then
			love.event.pump()
			for name, a,b,c,d,e,f in love.event.poll() do
				if name == "quit" then
					if not love.quit or not love.quit() then
						return a or 0
					end
				end
				love.handlers[name](a,b,c,d,e,f)
			end
		end
		prof.pop("events")

		-- Update dt, as we'll be passing it to update
		if love.timer then dt = love.timer.step() end

		-- Call update and draw
		prof.push("update")
		if love.update then love.update(dt) end -- will pass 0 if love.timer is disabled
		prof.pop("update")

		prof.push("draw")
		if love.graphics and love.graphics.isActive() then
			prof.push("clear")
			love.graphics.origin()
			love.graphics.clear(love.graphics.getBackgroundColor())
			prof.pop("clear")

			if love.draw then love.draw() end

			prof.push("present")
			love.graphics.present()
			prof.pop("present")
		end
		prof.pop("draw")
		prof.pop("frame")

		if love.timer then love.timer.sleep(0.001) end
	end
end
