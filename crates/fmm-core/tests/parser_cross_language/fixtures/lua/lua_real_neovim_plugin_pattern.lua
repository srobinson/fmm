local M = {}

local api = require("vim.api")
local fn = require("vim.fn")
local config = require("./config")

function M.setup(opts)
    opts = opts or {}
    M.config = vim.tbl_deep_extend("force", M.defaults, opts)
end

function M.run(args)
    if not M.config then
        error("Plugin not configured. Call setup() first.")
    end
    return M.config
end

function M.get_status()
    return { active = true, version = "1.0" }
end

local function validate_opts(opts)
    return type(opts) == "table"
end

local function apply_highlights()
    api.nvim_set_hl(0, "MyPluginHL", { fg = "white" })
end

return M
