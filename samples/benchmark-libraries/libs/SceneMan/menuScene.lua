-- Contains the scene, its callback functions, and its attribtue
local thisScene = {}

-- Declares variables that will be initialized when the scene is first loaded
-- Make sure you declare them up here so the other callback functions have access to them!
local sceneMan

local dt = 0

function thisScene:load (...)
    -- Scene Man is always the first item in the vararg (...)
    sceneMan = ...
end

function thisScene:delete ()
    print ("Help! I've been deleted from memory too!")
end

function thisScene:whenAdded ()
    print ("I have also been added to the stack!")
end

function thisScene:whenRemoved ()
    print ("I too was banished from the stack...")
end

function thisScene:update (dt)
    dt = dt -- Saves delta time to a local variable so it can be used inside the draw callback
end

function thisScene:draw (rng)
    -- Renders some GUI items
    love.graphics.setColor (1, 1, 1, 1)
    love.graphics.print ("Hello world! This is a GUI!", 400, 400)
    love.graphics.print ("Delta Time: " .. dt, 400, 425)
    love.graphics.print ("RNG: " .. sceneMan.shared.rngValue, 400, 450)
end

function thisScene:key (key)
	-- Pops a scene from the stack when R is pressed
    if key == "r" then
        print ("I'm getting removed from the stack now...")
        sceneMan:pop ()
    end
end

return thisScene