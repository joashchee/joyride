-- Contains the scene, its callback functions, and its attribtue
local thisScene = {}

-- Declares variables that will be initialized when the scene is first loaded
-- Make sure you declare them up here so the other callback functions have access to them!
local sceneMan, rectW

function thisScene:load (...)
    -- Scene Man is always the first item in the vararg (...)
    sceneMan, rectW = ...
end

function thisScene:delete ()
    print ("Help! I've been deleted from memory!")
end

function thisScene:whenAdded ()
    print ("I have been added to the stack!")
end

function thisScene:whenRemoved ()
    print ("I was banished from the stack...")
end

function thisScene:update (dt)
    -- We won't be using dt in this scene
end

function thisScene:draw (rng)
    love.graphics.setColor (1, 1, 1, 1)
    love.graphics.rectangle ("fill", 100, 100, rectW * rng, 100) -- Draws a rectangle of a random size

    sceneMan.shared.rngValue = rng -- Saves a value to the table shared between all the scenes
end

function thisScene:key (key)
	print ("Hey! This key was just pressed: " .. key)
end

return thisScene