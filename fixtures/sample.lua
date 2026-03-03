-- Sample Lua module demonstrating common patterns
local M = {}

-- External dependencies
local json = require("cjson")
local socket = require("socket")
local log = require("log")

-- Relative dependencies
local config = require("./config")
local utils = require("../lib/utils")

-- Module constants (not exported as functions, but part of the module table)
M.VERSION = "1.0.0"
M.MAX_RETRIES = 5

-- Private state
local internal_state = {}
local retry_count = 0

--- Initialize the module with the given configuration.
-- @param cfg table Configuration table
-- @return boolean Success status
function M.init(cfg)
    if not cfg then return false end
    internal_state.config = cfg
    return true
end

--- Process a batch of items.
-- @param items table Array of items to process
-- @return table Results
function M.process(items)
    local results = {}
    for i, item in ipairs(items) do
        local ok, result = pcall(transform_item, item)
        if ok then
            table.insert(results, result)
        end
    end
    return results
end

--- Transform input data.
-- @param input string Input data
-- @return string Transformed output
function M.transform(input)
    if type(input) ~= "string" then
        error("Expected string input")
    end
    return input:upper()
end

--- Get the current status.
-- @return table Status information
function M.status()
    return {
        initialized = internal_state.config ~= nil,
        retries = retry_count,
        version = M.VERSION,
    }
end

--- Reset the module state.
function M.reset()
    internal_state = {}
    retry_count = 0
end

-- Private helper functions (local, not exported)
local function validate_input(data)
    return type(data) == "table" and #data > 0
end

local function format_output(result)
    return json.encode(result)
end

local function log_action(action, details)
    log.info(string.format("[%s] %s", action, details))
end

-- Global utility function (exported as global, not on M)
function create_connection(host, port)
    return socket.connect(host, port)
end

-- Another global
function parse_config(path)
    local f = io.open(path, "r")
    if not f then return nil end
    local content = f:read("*a")
    f:close()
    return json.decode(content)
end

return M
