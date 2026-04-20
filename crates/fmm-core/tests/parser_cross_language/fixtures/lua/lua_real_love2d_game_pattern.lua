local physics = require("love.physics")
local graphics = require("love.graphics")

local world
local player = { x = 0, y = 0, speed = 200 }

function love_load()
    world = physics.newWorld(0, 9.81 * 64, true)
end

function love_update(dt)
    world:update(dt)
    player.x = player.x + player.speed * dt
end

function love_draw()
    graphics.print("Hello World", 400, 300)
    graphics.circle("fill", player.x, player.y, 20)
end

local function reset_player()
    player.x = 0
    player.y = 0
end

local function check_bounds(x, y)
    return x >= 0 and x <= 800 and y >= 0 and y <= 600
end
