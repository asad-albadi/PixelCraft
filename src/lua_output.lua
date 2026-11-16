-- Headless print output stays out of the process's JSON response stream.
local stringify, select_value, join = tostring, select, table.concat
local chunks, remaining, truncated = {}, 65536, false
local function append(text)
    if #text > remaining then
        text = text:sub(1, remaining)
        truncated = true
    end
    if #text > 0 then
        chunks[#chunks + 1] = text
        remaining = remaining - #text
    end
end

print = function(...)
    for i = 1, select_value('#', ...) do
        if remaining == 0 then
            truncated = true
            return
        end
        if i > 1 then append('\t') end
        append(stringify(select_value(i, ...)))
    end
    append('\n')
end

function _pw_take_output()
    local output = join(chunks)
    if truncated then output = output .. '\n[Lua output truncated]\n' end
    chunks, remaining, truncated = {}, 65536, false
    return output
end
