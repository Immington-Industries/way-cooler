-- Lua configuration file for way-cooler.

--
-- Layouts
--

--[[ Layout code is not implemented at this time, sorry.

-- The default layout options are no names, mode = "default" (use keybindings).
-- For a list of tiling options, see way-cooler docs or `man way-cooler-tiling`.
-- Workspaces, like arrays in Lua, start with 1.
local workspace_settings = {
  -- The first workspace is named web
  [1] = { name = "web" },
  -- The 9th workspace is named "free", and all windows sent there float.
  [9] = { name = "free", mode = "float" },
}

-- Create 9 workspaces with the given settings.
config.init_workspaces(workspace_settings) -- Not implemented yet

]]

--
-- Background
--
--
-- A background can either be a 6 digit hex value or an image path (not yet supported)
way_cooler.background = "0x8A2ACA"

--
-- Keybindings
--
-- Create an array of keybindings and call config.register_keys()
-- to register them.
-- Declaring a keybinding:
-- key(<modifiers list>, <key>, <function or name>, [repeat])

-- <modifiers list>: Modifiers (mod4, shift, control) to be used

-- <key>: Name of the key to be pressed. See xkbcommon keysym names.

-- <function or name> If a string, the way-cooler command to be run.
-- If a function, a Lua function to run on the keypress. The function takes
-- a list of key names as input (i.e. { "mod4", "shift", "a" }) if needed.

-- [repeat]: Optional boolean defaults to true - if false, the command will
-- will not follow "hold down key to repeat" rules, and will only run once,
-- waiting until the keys are released to run again.

-- Modifier key used in keybindings. Mod3 = Alt, Mod4 = Super/Logo key
mod = "Alt"
local key = config.key -- Alias key so it's faster to type

way_cooler.terminal = "weston-terminal" -- Use the terminal of your choice

local keys = {
  -- Open dmenu
  key({ mod }, "d", "launch_dmenu"),
  -- Open terminal
  key({ mod }, "return", "launch_terminal"),

  -- Lua methods can be bound as well
  key({ mod, "Shift" }, "h", function () print("Hello world!") end),

  -- Some Lua dmenu stuff
  key({ mod }, "l", "dmenu_eval"),
  key({ mod, "Shift" }, "l", "dmenu_lua_dofile"),

  -- Move focus
  -- (if you're used to mod+arrow to switch workspaces you may wanna change them)
  key({ mod }, "left", "focus_left"),
  key({ mod }, "right", "focus_right"),
  key({ mod }, "up", "focus_up"),
  key({ mod }, "down", "focus_down"),

  -- Split containers
  key({ mod }, "h", "split_horizontal"),
  key({ mod }, "v", "split_vertical"),
  key({ mod }, "e", "horizontal_vertical_switch"),
  key({ mod }, "q", "close_window"),

  -- Quitting way-cooler is hardcoded to Alt+Shift+Esc.
  -- This my be modifiable in the future
}

-- Add Mod + X bindings to switch to workspace X, Mod+Shift+X send active to X
for i = 1, 9 do
  table.insert(keys,
               key({ mod }, tostring(i), "switch_workspace_" .. i))
  table.insert(keys,
               key({ mod, "Shift" }, tostring(i), "move_to_workspace_" .. i))
end

-- Register the keybindings.
for _, key in pairs(keys) do
    config.register_key(key)
end

-- To use plugins such as bars, or to start other programs on startup,
-- call util.exec.spawn_once, which will not spawn copies after a config reload.

-- util.exec.spawn_once("way-cooler-bar")

-- To add your own Lua files:
-- require("my-config.lua") -- Or use utils.hostname

-- !! Do not place any code after this comment.
-- !! way-cooler and plugins may insert auto-generated code.
