export const STARTER = `function love.load()
  x = 100
end

function love.update(dt)
  x = x + 60 * dt
end

function love.draw()
  love.graphics.setColor(1, 0.4, 0.66)
  love.graphics.rectangle("fill", x, 100, 50, 50)
end
`;
