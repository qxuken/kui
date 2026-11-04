-- Generated from src/syntax.tl by addons/kit/build.lua. Edit the .tl.


local M = {}
local WORDS = " and as async await auto bool break case catch char class const continue def default do else elif elseif end enum export extern false fn for from function if impl import in interface let local match mod mut new nil None not null or pass pub raise record return self static string struct super switch then this throw trait true try type typeof union use var void while with yield "
local LANGS = { lua = true, teal = true, javascript = true, typescript = true,
json = true, rust = true, python = true, c = true, css = true, shell = true, sql = true, }

function M.lines(source, language, accent, muted, literal)
   local out = { {} }
   local function add(s, color)
      local at = 1
      while true do
         local e = s:find("\n", at, true)
         local chunk = s:sub(at, e and e - 1 or #s)
         if #chunk > 0 then out[#out][#out[#out] + 1] = color and { chunk, color = color } or chunk end
         if not e then break end
         out[#out + 1] = {}
         at = e + 1
      end
   end
   if not LANGS[language] then add(source); return out end
   local lua = language == "lua" or language == "teal"
   local hash = language == "python" or language == "shell"
   local at = 1
   while at <= #source do
      local tail = source:sub(at)
      local c = source:sub(at, at)
      local next2 = source:sub(at, at + 1)
      local chunk
      local color
      if (lua and next2 == "--") or (not lua and not hash and next2 == "//") or (hash and c == "#") or
         (language == "sql" and next2 == "--") then
         if lua and tail:sub(1, 4) == "--[[" then
            local e = source:find("]]", at + 4, true)
            chunk = source:sub(at, e and e + 1 or #source)
         else chunk = tail:match("^[^\n]*") end
         color = muted
      elseif not lua and not hash and next2 == "/*" then
         local e = source:find("*/", at + 2, true)
         chunk, color = source:sub(at, e and e + 1 or #source), muted
      elseif lua and next2 == "[[" then
         local e = source:find("]]", at + 2, true)
         chunk, color = source:sub(at, e and e + 1 or #source), literal
      elseif c == '"' or c == "'" or c == "`" then
         local j = at + 1
         while j <= #source do
            local b = source:sub(j, j)
            if b == "\\" then j = j + 2
            elseif b == c then j = j + 1; break
            else j = j + 1 end
         end
         chunk, color = source:sub(at, j - 1), literal
      elseif c:match("[%a_]") then
         chunk = tail:match("^[%w_]+")
         local word = language == "sql" and chunk:lower() or chunk
         if WORDS:find(" " .. word .. " ", 1, true) then color = accent end
      elseif c:match("%d") then
         chunk, color = tail:match("^[%w_.]+"), literal
      else chunk = c end
      add(chunk, color)
      at = at + #chunk
   end
   return out
end

return M