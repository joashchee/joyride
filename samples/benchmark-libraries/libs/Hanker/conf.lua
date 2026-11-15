if love.filesystem then
	require 'rocks' ()
end
function love.conf(t)
	t.identity ="anchor"
	t.window.vsync = false
	t.window.resizable = true
	t.dependencies = {
		"love-imgui",
	}
end
