-- Contains the scene, its callback functions, and its attribtue
local thisScene = {}

-- Declares variables that will be initialized when the scene is first loaded
-- Make sure you declare them up here so the other callback functions have access to them!
local sceneMan

function thisScene:load (...)
    -- Scene Man is always the first item in the vararg (...)
    sceneMan = ...
end

function thisScene:delete ()
    
end

function thisScene:whenAdded ()
    
end

function thisScene:whenRemoved ()
    
end

-- You can place your custom event callback functions here!
-- Ex: function thisScene:update (dt)

return thisScene