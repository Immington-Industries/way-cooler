--------------------
-- Module with utility functions
--
-- @module utils
--
-- Submodules:
--
-- * `file`: Operations on files
-- * `math`: Contains a few math functions
-- * `string`: Operations on strings
-- * `table`: Operations on tables

local util = {}

-- IO Functions

util.file = {}

-- Opens the file at path with the given mode and format
-- @param path File to be opened
-- @param mode Optional mode to open file with
-- @param format Optional format to read file with
-- @return The content of the file
function util.file.read_all(path, mode, format)
    assert(path ~= nil, "File path was nil")
    if mode == nil then
        mod = "*all"
    end
    local file = io.open(path)
    if file == nil then
        error("Unable to open " .. path .. "!", 2)
    end
    local data = file:read(format)
    file:close()
    return data
end

-- Math functions
util.math = {}



-- Converts a number in a range to a percentage
-- @param min The minimum value in the range
-- @param max The maximum value in the range
-- @param value The value in the range to convert
-- @return A percentage from 0 to 100 for the value
function util.math.range_to_percent(min, max, value)
    assert(type(min) == 'number', "min: expected number")
    assert(type(max) == 'number', "max: expected number")
    assert(type(value) == 'number', "value: expected number")
    assert(min < max, "min value was not less than max!")

    value = math.min(max, value)
    value = math.max(min, value)

    return math.ceil( (value - min) / (max - min) * 100 )
end

-- String functions

util.string = {}

-- Counts the number of lines in a string.
-- @param text String to count lines of
-- @return The number of lines in the string.
function util.string.line_count(text)
    assert(type(text) == 'string', "Non-string given to string.line_count!")
    local count = 0
    for result in text:gmatch("\n") do
        count = count + 1
    end
    return count
end

-- Escapes backslashes and quotes in a string.
--
-- Replaces " with \", ' with \', and \ with \\.
-- @param text String to escape
-- @return String escaped with quotes.
function util.string.escape_quotes(text)
    assert(type(text) == 'string', "string.escape: Expected a string")
    text = text:gsub('\\', '\\\\')
    text = text:gsub('"', '\\"')
    text = text:gsub("'", "\\'")
    return text
end

-- Escapes strings for HTML encoding.
--
-- Replaces <, >, &, ", and ' with their HTML &name; equivalents.
-- @param test The text to escape
-- @return HTML escaped text.
function util.string.escape_html(text)
    assert(type(text) == 'string', "string.html_escape: Expected a string")
    builder = ""
    for i = 1, text:len() do
        if char == '<' then
            builder = builder + '&lt;'
        elseif char == '>' then
            builder = builder + '&gt;'
        elseif char == '&' then
            builder = builder + '&amp;'
        elseif char == '"' then
            builder = builder + '&quot;'
        elseif char == "'" then
            builder = builder + '&apos;'
        else
            builder = builder + text[i]
        end
    end
    return builder
end

-- Table functions

util.table = {}

-- Gets a random element from a numerically-indexed list.
--
-- # Errors
-- Function will error if the table is nil or empty,
-- or if the indicies are not numbers.
--
-- @param tab The list to pick from
-- @return A random element from the list
function util.table.get_random(tab)
    assert(type(tab) == 'table', "Non table given to table.get_random!")
    local len = #tab
    if len == 0 then
        error("Empty table given to table.get_random!", 2)
    elseif len == 1 then
        return tab[1]
    else
        return tab[math.random(1, len)]
    end
end

return util
