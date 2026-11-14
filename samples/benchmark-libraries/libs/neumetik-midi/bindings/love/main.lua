-- A LÖVE game with music by Neumetik MIDI. Put song.mid (try the demo's
-- overworld.mid) and the library beside this file, then run `love .`.
--
--   Space   pause / play        M   mute the drums (channel 9)
--   F       fade out / in       1-8 play a note over the music

local NM = require("neumetik_midi")

local music
local faded = false
local drums_muted = false

function love.load()
  music = NM.new()
  local ok, why = music:load(assert(love.filesystem.read("song.mid"), "Put a song.mid beside main.lua."))
  assert(ok, why)
  music:setLooping(true)
  music:play()
  -- A free channel for the notes played over the music: a vibraphone.
  music:setInstrument(15, 0, 11)
end

function love.update(dt)
  music:update()
end

function love.keypressed(key)
  if key == "space" then
    if music:isPlaying() then music:pause() else music:play() end
  elseif key == "f" then
    faded = not faded
    music:fadeTo(faded and 0 or 1, 1.5)
  elseif key == "m" then
    drums_muted = not drums_muted
    music:setChannelMuted(9, drums_muted)
  elseif tonumber(key) then
    local scale = { 60, 62, 64, 67, 69, 72, 74, 76 }
    local note = scale[tonumber(key)]
    if note then music:noteOn(15, note, 110) end
  end
end

function love.keyreleased(key)
  local scale = { 60, 62, 64, 67, 69, 72, 74, 76 }
  local note = tonumber(key) and scale[tonumber(key)]
  if note then music:noteOff(15, note) end
end

function love.draw()
  local start, finish = music:getLoopPoints()
  love.graphics.print(string.format(
    "%s  %.1f / %.1f s  (loop %.1f to %.1f)\nvoices %d   lead: %s\n\nSpace pause, F fade, M mute drums, 1-8 notes",
    music:state(), music:tell(), music:getDuration(), start, finish, music:activeVoices(), music:instrumentName(3)), 20, 20)
end
