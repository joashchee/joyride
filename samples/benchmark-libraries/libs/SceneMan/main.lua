local sceneMan = require ("sceneMan")

function love.load ()
    local initialRectW = 300 -- Initial size of the gameplay rectangle

	sceneMan:newScene ("game", require ("gameScene"), initialRectW) -- Here we pass the initial rectangle width into the scene
	sceneMan:newScene ("menu", require ("menuScene")) -- This scene doesn't need any extra values, so we don't pass anything

    -- This is where the fun begins!
    sceneMan:push ("game")
    sceneMan:push ("menu")
end

function love.update (dt)
	-- We pass dt from love.update into each scene
	sceneMan:event ("update", dt)
end

function love.draw ()
    local rng = math.random ()
    
    -- We can pass any values we desire into these events
    sceneMan:event ("draw", rng)
end

function love.keypressed (key)
	-- The event name can also be anything we want, so long as we use the same name inside the scenes we define
	sceneMan:event ("key", key)

    -- This demonstrates the stack saving and stack locking systems
    if key == "q" then
        sceneMan:saveStack ("test") -- Saves the current stack with id="test"
    elseif key == "w" then
        print (sceneMan:restoreStack ("test")) -- Restores the stack stored with id="test"
    elseif key == "e" then
        sceneMan:deleteStack ("test") -- Deletes the stack stored with id="test"
    elseif key == "r" then
        sceneMan:clearStack () -- Clears the stack. This is required, otherwise the restoreStack method will fail
    elseif key == "o" then
        sceneMan:lock (1) -- Locks the stack up to level 1 (gameScene in this case)
    elseif key == "p" then
        sceneMan:unlock () -- Unlocks the stack
    end
end