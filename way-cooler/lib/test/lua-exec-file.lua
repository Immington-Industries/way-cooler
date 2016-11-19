-- Lua tests for the Lua thread

local foo = "foo"
local bar = 'bar'

assert(1 == 1)

function confirm_file()
  return "File loaded"
end
