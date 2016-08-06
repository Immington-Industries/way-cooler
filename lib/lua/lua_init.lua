-- Private table of Rust functions
local rust = __rust
__rust = nil

-- Initialize the workspaces
config.init_workspaces = function(settings)
    assert(type(settings) == 'table', "settings: expected table")
    for ix, val in pairs(settings) do
        assert(type(ix) == 'number', "settings: expected number-indexed array")
        assert(type(val) == 'table', "settings: expected array of tables")

        val.name = val.name or ""
        val.mode = val.mode or "tiling"
    end
    rust.init_workspaces(settings)
end

-- Create a new keybinding to register with Rust
config.key = function(mods, key, action, loop)
    assert(type(mods) == 'table', "modifiers: expected table")
    assert(type(key) == 'string', "key: expected string")
    if loop == nil then loop = true end
    if type(action) ~= 'string' and type(action) ~= 'function' then
        error("action: expected string or function", 2)
    end
    return {
        mods = mods, key = key, action = action, loop = loop
    }
end

local use_key = ", use the `key` or `config.key` method to create a keybinding"

-- Converts a list of modifiers to a string
local function keymods_to_string(mods, key)
    table.insert(mods, key)
    return table.concat(mods, ',')
end

-- Save the action at the __key_map and tell Rust to register the Lua key
local function register_lua_key(index, action, loop)
    local map_ix = rust.register_lua_key(index, loop)
    __key_map[map_ix] = action
end

-- Register a keybinding
config.register_key = function(key)
    assert(key.mods, "keybinding missing modifiers" .. use_key)
    assert(key.key, "keybinding missing modifiers" .. use_key)
    assert(key.action, "keybinding missing action" .. use_key)
    assert(key.loop, "keybinding missing repeat" .. use_key)
    assert(type(key.mods) == 'table',
           "keybinding modifiers: expected table" .. use_key)
    assert(type(key.key) == 'string',
           "keybinding key: expected string" .. use_key)
    assert(type(key.loop) == 'boolean',
           "keybinding repeat: expected optional boolean" .. use_key)

    if (type(key.action) == 'string') then
        rust.register_command_key(keymods_to_string(key.mods, key.key),
                                  key.action, key.loop)
    elseif (type(key.action) == 'function') then
        register_lua_key(keymods_to_string(key.mods, key.key),
                              key.action, key.loop)
    else
        error("keybinding action: expected string or a function"..use_key, 2)
    end
end
