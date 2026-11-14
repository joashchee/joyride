-- Neumetik MIDI for LÖVE (and any LuaJIT): General MIDI and GS music, no
-- SoundFont. Copyright 2026 ansiapps. MIT licensed.
--
-- Put this file and the library for each platform beside main.lua:
-- libneumetik_midi.dylib (macOS), libneumetik_midi.so (Linux),
-- neumetik_midi.dll (Windows).
--
--   local NM = require("neumetik_midi")
--   local music = NM.new()                 -- 44100 Hz, a LÖVE QueueableSource
--   music:load(love.filesystem.read("song.mid"))
--   music:setLooping(true)
--   music:play()
--   function love.update(dt) music:update() end   -- keeps the source fed
--
-- To play through a SoundFont or DLS bank, before playing:
--   music:loadSoundBank(love.filesystem.read("bank.sf2"))
--
-- Without LÖVE, NM.player(rate) gives the bare player: render into your
-- own buffers.

local ffi = require("ffi")

ffi.cdef [[
typedef struct nm_player nm_player;
typedef struct nm_soundbank nm_soundbank;
uint32_t nm_version(void);
nm_player *nm_create(uint32_t sample_rate);
void nm_destroy(nm_player *p);
uint32_t nm_sample_rate(const nm_player *p);
int32_t nm_load(nm_player *p, const uint8_t *data, size_t size);
const char *nm_error_string(int32_t code);
void nm_unload(nm_player *p);
int32_t nm_load_soundbank(nm_player *p, const uint8_t *data, size_t size);
void nm_set_soundbank(nm_player *p, const nm_soundbank *bank);
void nm_play(nm_player *p);
void nm_pause(nm_player *p);
void nm_stop(nm_player *p);
int32_t nm_state(const nm_player *p);
int32_t nm_is_finished(const nm_player *p);
void nm_seek(nm_player *p, double seconds);
double nm_position(const nm_player *p);
double nm_duration(const nm_player *p);
void nm_set_looping(nm_player *p, int32_t looping);
int32_t nm_is_looping(const nm_player *p);
void nm_set_loop_points(nm_player *p, double start, double end);
double nm_loop_start(const nm_player *p);
double nm_loop_end(const nm_player *p);
void nm_set_speed(nm_player *p, double speed);
void nm_set_volume(nm_player *p, float volume);
float nm_volume(const nm_player *p);
void nm_fade_to(nm_player *p, float volume, float seconds);
void nm_set_channel_muted(nm_player *p, int32_t channel, int32_t muted);
void nm_note_on(nm_player *p, int32_t channel, int32_t key, int32_t velocity);
void nm_note_off(nm_player *p, int32_t channel, int32_t key);
void nm_control_change(nm_player *p, int32_t channel, int32_t controller, int32_t value);
void nm_program_change(nm_player *p, int32_t channel, int32_t program);
void nm_set_instrument(nm_player *p, int32_t channel, int32_t bank, int32_t program);
void nm_pitch_bend(nm_player *p, int32_t channel, int32_t value);
void nm_send_midi(nm_player *p, const uint8_t *data, size_t size);
void nm_all_notes_off(nm_player *p);
void nm_all_sound_off(nm_player *p);
void nm_reset(nm_player *p);
void nm_render_f32(nm_player *p, float *out, size_t frames);
void nm_render_s16(nm_player *p, int16_t *out, size_t frames);
void nm_render_f32_planar(nm_player *p, float *left, float *right, size_t frames);
int32_t nm_active_voices(const nm_player *p);
int32_t nm_instrument_name(const nm_player *p, int32_t channel, char *buffer, size_t size);
]]

-- Finds the library: beside this file, in LÖVE's source or save folder,
-- or where the system looks.
local function open_library()
  local names = {
    OSX = "libneumetik_midi.dylib",
    Linux = "libneumetik_midi.so",
    Windows = "neumetik_midi.dll",
  }
  local file = names[ffi.os] or "libneumetik_midi.so"
  local tried = {}
  local places = { "./" .. file }
  local here = debug.getinfo(1, "S").source:match("^@(.*[/\\])")
  if here then table.insert(places, 1, here .. file) end
  if love and love.filesystem then
    table.insert(places, love.filesystem.getSourceBaseDirectory() .. "/" .. file)
    table.insert(places, love.filesystem.getSaveDirectory() .. "/" .. file)
  end
  table.insert(places, "neumetik_midi")
  for _, place in ipairs(places) do
    local ok, lib = pcall(ffi.load, place)
    if ok then return lib end
    table.insert(tried, place)
  end
  error("Couldn't find the Neumetik MIDI library (" .. file .. "). Looked in: " .. table.concat(tried, ", "))
end

local C = open_library()
local STATES = { [0] = "stopped", "playing", "paused" }

local Player = {}
Player.__index = Player

local M = { version = C.nm_version() }

--- A bare player at `rate` frames a second.
function M.player(rate)
  local p = C.nm_create(rate or 44100)
  return setmetatable({ p = ffi.gc(p, C.nm_destroy), name = ffi.new("char[64]") }, Player)
end

--- Loads a MIDI file from a string of its bytes. Returns true, or false
--- and why not.
function Player:load(bytes)
  local code = C.nm_load(self.p, ffi.cast("const uint8_t *", bytes), #bytes)
  if code ~= 0 then return false, ffi.string(C.nm_error_string(code)) end
  return true
end

--- Plays through a SoundFont (.sf2) or DLS (.dls) bank, from a string of
--- its bytes: its instruments, and Neumetik's for what it hasn't. Returns
--- true, or false and why not.
function Player:loadSoundBank(bytes)
  local code = C.nm_load_soundbank(self.p, ffi.cast("const uint8_t *", bytes), #bytes)
  if code ~= 0 then return false, ffi.string(C.nm_error_string(code)) end
  return true
end

--- Back to Neumetik's own instruments.
function Player:clearSoundBank() C.nm_set_soundbank(self.p, nil) end

function Player:unload() C.nm_unload(self.p) end
function Player:play() C.nm_play(self.p) end
function Player:pause() C.nm_pause(self.p) end
function Player:stop() C.nm_stop(self.p) end
--- "stopped", "playing" or "paused".
function Player:state() return STATES[C.nm_state(self.p)] end
function Player:isPlaying() return C.nm_state(self.p) == 1 end
function Player:isFinished() return C.nm_is_finished(self.p) ~= 0 end
function Player:seek(seconds) C.nm_seek(self.p, seconds) end
function Player:tell() return C.nm_position(self.p) end
function Player:getDuration() return C.nm_duration(self.p) end
function Player:setLooping(on) C.nm_set_looping(self.p, on and 1 or 0) end
function Player:isLooping() return C.nm_is_looping(self.p) ~= 0 end
--- In seconds; nil for either goes back to the file's own loop points.
function Player:setLoopPoints(start, finish) C.nm_set_loop_points(self.p, start or -1, finish or -1) end
function Player:getLoopPoints() return C.nm_loop_start(self.p), C.nm_loop_end(self.p) end
function Player:setSpeed(speed) C.nm_set_speed(self.p, speed) end
function Player:setVolume(volume) C.nm_set_volume(self.p, volume) end
function Player:getVolume() return C.nm_volume(self.p) end
function Player:fadeTo(volume, seconds) C.nm_fade_to(self.p, volume, seconds) end
function Player:setChannelMuted(channel, muted) C.nm_set_channel_muted(self.p, channel, muted and 1 or 0) end

function Player:noteOn(channel, key, velocity) C.nm_note_on(self.p, channel, key, velocity or 100) end
function Player:noteOff(channel, key) C.nm_note_off(self.p, channel, key) end
function Player:controlChange(channel, controller, value) C.nm_control_change(self.p, channel, controller, value) end
function Player:programChange(channel, program) C.nm_program_change(self.p, channel, program) end
function Player:setInstrument(channel, bank, program) C.nm_set_instrument(self.p, channel, bank, program) end
function Player:pitchBend(channel, value) C.nm_pitch_bend(self.p, channel, value) end
function Player:sendMidi(bytes) C.nm_send_midi(self.p, ffi.cast("const uint8_t *", bytes), #bytes) end
function Player:allNotesOff() C.nm_all_notes_off(self.p) end
function Player:allSoundOff() C.nm_all_sound_off(self.p) end
function Player:reset() C.nm_reset(self.p) end
function Player:activeVoices() return C.nm_active_voices(self.p) end
function Player:instrumentName(channel)
  local n = C.nm_instrument_name(self.p, channel, self.name, 64)
  return ffi.string(self.name, math.min(n, 63))
end

--- Renders `frames` frames of interleaved 16-bit stereo into `pointer`
--- (an int16_t*, such as SoundData:getFFIPointer()).
function Player:renderS16(pointer, frames) C.nm_render_s16(self.p, ffi.cast("int16_t *", pointer), frames) end
function Player:renderF32(pointer, frames) C.nm_render_f32(self.p, ffi.cast("float *", pointer), frames) end

--- Music for LÖVE: a player feeding a QueueableSource. Call update() from
--- love.update; it keeps `buffers` buffers of `frames` frames queued
--- (by default 4 of 1024 at 44.1 kHz: about 93 ms ahead).
function M.new(rate, frames, buffers)
  assert(love and love.audio, "neumetik_midi.new needs LÖVE; use neumetik_midi.player for a bare player")
  rate, frames, buffers = rate or 44100, frames or 1024, buffers or 4
  local music = M.player(rate)
  music.frames = frames
  music.source = love.audio.newQueueableSource(rate, 16, 2, buffers)
  music.pool = {}
  for i = 1, buffers do music.pool[i] = love.sound.newSoundData(frames, rate, 16, 2) end
  music.next = 1
  local play = Player.play
  function music:play()
    play(self)
    self:update()
    self.source:play()
  end
  function music:update()
    while self.source:getFreeBufferCount() > 0 do
      local data = self.pool[self.next]
      self.next = self.next % #self.pool + 1
      self:renderS16(data:getFFIPointer(), self.frames)
      self.source:queue(data)
    end
    if not self.source:isPlaying() then self.source:play() end
  end
  return music
end

return M
