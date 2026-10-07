local utf8 = require("lua-utf8")

-- Test helpers

local tests = 0

local function check(ok, msg)
  tests = tests + 1
  if not ok then
    error(msg or "assertion failed", 2)
  end
end

local function eq(actual, expected, msg)
  tests = tests + 1
  if actual ~= expected then
    error(
      (msg or "values differ")
      .. "\n expected: " .. tostring(expected)
      .. "\n actual: " .. tostring(actual),
      2
    )
  end
end

local function raises(f, msg)
  tests = tests + 1
  local ok = pcall(f)
  if ok then
    error(msg or "expected an error", 2)
  end
end

local function collect_codes(s, lax)
  local r = {}
  for p, cp in utf8.codes(s, lax) do
    r[#r + 1] = { p, cp }
  end
  return r
end

local function assert_array(a, b, msg)
  eq(#a, #b, msg or "array length differs")
  for i = 1, #a do
    if type(a[i]) == "table" then
      assert_array(a[i], b[i], (msg or "array") .. "[" .. i .. "]")
    else
      eq(a[i], b[i], (msg or "array") .. "[" .. i .. "]")
    end
  end
end


-- Test string: "Aé中🙂B"

-- character indices:
-- 1 A
-- 2 é
-- 3 中
-- 4 🙂
-- 5 B

-- byte indices:
-- 1 A
-- 2 é
-- 4 中
-- 7 🙂
-- 11 B

-- byte length = 11
-- character length = 5

local S = "Aé中🙂B"
local EMPTY = ""

local A = "A"
local E_ACUTE = "é"
local CJK = "中"
local EMOJI = "🙂"
local B = "B"

eq(#S, 11, "fixture byte length")
eq(utf8.len(S), 5, "fixture character length")

---

-- utf8.byte
do
  eq(utf8.byte(S), 0x41)
  eq(utf8.byte(S, 1), 0x41)
  eq(utf8.byte(S, 2), 0xE9)
  eq(utf8.byte(S, 3), 0x4E2D)
  eq(utf8.byte(S, 4), 0x1F642)
  eq(utf8.byte(S, 5), 0x42)

  -- zero and negative character indices
  eq(utf8.byte(S, 0), nil)
  eq(utf8.byte(S, -1), 0x42)
  eq(utf8.byte(S, -2), 0x1F642)
  eq(utf8.byte(S, -3), 0x4E2D)
  eq(utf8.byte(S, -4), 0xE9)
  eq(utf8.byte(S, -5), 0x41)
  eq(utf8.byte(S, -6), nil)

  -- ranges
  local a, b, c, d, e = utf8.byte(S, 1, 5)
  eq(a, 0x41)
  eq(b, 0xE9)
  eq(c, 0x4E2D)
  eq(d, 0x1F642)
  eq(e, 0x42)

  a, b, c, d, e = utf8.byte(S, -5, -1)
  eq(a, 0x41)
  eq(b, 0xE9)
  eq(c, 0x4E2D)
  eq(d, 0x1F642)
  eq(e, 0x42)

  a, b, c, d, e = utf8.byte(S, 0, 3)
  eq(a, 0x41)
  eq(b, 0xE9)
  eq(c, 0x4E2D)
  eq(d, nil)
  eq(e, nil)

  -- empty string
  eq(utf8.byte(EMPTY), nil)
  eq(utf8.byte(EMPTY, 1), nil)
  eq(utf8.byte(EMPTY, -1), nil)

  -- empty and out-of-range ranges return no values
  eq(select("#", utf8.byte(EMPTY)), 0)
  eq(select("#", utf8.byte(S, 3, 2)), 0)
  eq(select("#", utf8.byte(S, 100, 200)), 0)
  eq(select("#", utf8.byte(S, -100, -200)), 0)

  -- NUL bytes survive a byte -> char round trip.
  local nul = "\0x"
  eq(string.char(utf8.byte(nul, 1, -1)), nul)
end

---

-- utf8.char
do
  eq(utf8.char(), "")
  eq(utf8.char(0), "\0")

  eq(utf8.char(0x41), "A")
  eq(utf8.char(0xE9), "é")
  eq(utf8.char(0x4E2D), "中")
  eq(utf8.char(0x1F642), "🙂")
  eq(utf8.char(0x42), "B")

  eq(utf8.char(0, 1, 2, 3), "\0\1\2\3")
  eq(utf8.char(0x41, 0xE9, 0x4E2D, 0x1F642, 0x42), S)

  -- doesn't error on surrogate code points
  eq(utf8.char(0xD800), "\xED\xA0\x80")
  eq(utf8.char(0xDFFF), "\xED\xBF\xBF")

  -- errors on code points outside range
  raises(function() utf8.char(-1) end)
  raises(function() utf8.char(0x110000) end)

  -- encoding boundaries for the 1- to 4-byte forms
  eq(utf8.char(0x7F), "\x7F")
  eq(utf8.char(0x80), "\xC2\x80")
  eq(utf8.char(0x7FF), "\xDF\xBF")
  eq(utf8.char(0x800), "\xE0\xA0\x80")
  eq(utf8.char(0xFFFF), "\xEF\xBF\xBF")
  eq(utf8.char(0x10000), "\xF0\x90\x80\x80")
  eq(utf8.char(0x10FFFF), "\xF4\x8F\xBF\xBF")
end

---

-- utf8.len
do
  eq(utf8.len(S), 5)
  eq(utf8.len(EMPTY), 0)

  -- exact character boundaries
  eq(utf8.len(S, 1, 11), 5)  -- full
  eq(utf8.len(S, 1, 1), 1)   -- A
  eq(utf8.len(S, 2, 3), 1)   -- é
  eq(utf8.len(S, 4, 6), 1)   -- 中
  eq(utf8.len(S, 7, 10), 1)  -- 🙂
  eq(utf8.len(S, 11, 11), 1) -- B

  -- empty byte ranges
  eq(utf8.len(S, 1, 0), 0)
  eq(utf8.len(S, 5, 4), 0)

  -- negative byte indices.
  eq(utf8.len(S, -1, -1), 1)
  eq(utf8.len(S, -5, -1), 2)
  eq(utf8.len(S, -11, -1), 5)

  -- zero is invalid as a byte position
  raises(function() utf8.len(S, 0) end)
  raises(function() utf8.len(S, 0, 11) end)

  -- starting in the middle of a UTF-8 sequence
  local r, e = utf8.len(S, 3, 3)
  eq(r, nil)
  eq(e, 3)

  r, e = utf8.len(S, 5, 10)
  eq(r, nil)
  eq(e, 5)

  -- invalid UTF-8
  local bad1 = "\x80"
  local bad2 = "\xC0\x80"
  local bad3 = "\xE2\x28\xA1"
  local bad4 = "\xF0\x28\x8C\xBC"
  local bad5 = "\xED\xA0\x80"

  local n, p = utf8.len(bad1)
  eq(n, nil)
  eq(p, 1)

  n, p = utf8.len(bad2)
  eq(n, nil)
  eq(p, 1)

  n, p = utf8.len(bad3)
  eq(n, nil)
  eq(p, 1)

  n, p = utf8.len(bad4)
  eq(n, nil)
  eq(p, 1)

  n, p = utf8.len(bad5)
  eq(n, nil)
  eq(p, 1)

  -- lax mode
  check(utf8.len(bad1, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad2, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad3, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad4, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad5, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")

  -- an end beyond the last byte is rejected.
  raises(function() utf8.len(S, 2, 100) end)
  raises(function() utf8.len(S, 12, 12) end)
end

---

-- utf8.sub
do
  eq(utf8.sub(S, 1), S)

  eq(utf8.sub(S, 1, 1), A)
  eq(utf8.sub(S, 2, 2), E_ACUTE)
  eq(utf8.sub(S, 3, 3), CJK)
  eq(utf8.sub(S, 4, 4), EMOJI)
  eq(utf8.sub(S, 5, 5), B)

  eq(utf8.sub(S, 1, 5), S)
  eq(utf8.sub(S, 2, 4), "é中🙂")
  eq(utf8.sub(S, 3, 5), "中🙂B")

  eq(utf8.sub(S, -1), B)
  eq(utf8.sub(S, -2), "🙂B")
  eq(utf8.sub(S, -5), S)
  eq(utf8.sub(S, -4, -2), "é中🙂")
  eq(utf8.sub(S, -3, -1), "中🙂B")

  eq(utf8.sub(S, 0, 3), "Aé中")
  eq(utf8.sub(S, 6), "")
  eq(utf8.sub(S, -6), S)
  eq(utf8.sub(S, 4, 2), "")
  eq(utf8.sub(S, 2, -5), "")
  eq(utf8.sub(EMPTY, 1), "")
  eq(utf8.sub(EMPTY, -1), "")

  -- out-of-range indices clamp like string.sub
  eq(utf8.sub(S, -100), S)
  eq(utf8.sub(S, 100), EMPTY)
  eq(utf8.sub(S, 1, -100), EMPTY)
  eq(utf8.sub(S, -1, 100), B)
  eq(utf8.sub(S, 0), S)
  eq(utf8.sub(S, 1, 0), EMPTY)

  local sub_expected = {
    [-6] = "",
    [-5] = "A",
    [-4] = "é",
    [-3] = "中",
    [-2] = "🙂",
    [-1] = "B",
    [0] = "",
    [1] = "A",
    [2] = "é",
    [3] = "中",
    [4] = "🙂",
    [5] = "B",
    [6] = "",
  }

  for i = -6, 6 do
    eq(utf8.sub(S, i, i), sub_expected[i],
      "utf8.sub single-index boundary " .. i)
  end
end

---

-- utf8.find
do
  local a, b = utf8.find(S, A)
  eq(a, 1)
  eq(b, 1)

  a, b = utf8.find(S, E_ACUTE)
  eq(a, 2)
  eq(b, 2)

  a, b = utf8.find(S, CJK)
  eq(a, 3)
  eq(b, 3)

  a, b = utf8.find(S, EMOJI)
  eq(a, 4)
  eq(b, 4)

  a, b = utf8.find(S, B)
  eq(a, 5)
  eq(b, 5)

  -- literal search
  a, b = utf8.find(S, "é中")
  eq(a, 2)
  eq(b, 3)

  -- empty pattern
  a, b = utf8.find(S, "")
  eq(a, 1)
  eq(b, 0)

  -- returns nil if not found
  eq(utf8.find(S, "not present"), nil)

  -- positive character-index init
  a, b = utf8.find(S, A, 1)
  eq(a, 1)
  eq(b, 1)

  a, b = utf8.find(S, A, 2)
  eq(a, nil)
  eq(b, nil)

  a, b = utf8.find(S, B, 5)
  eq(a, 5)
  eq(b, 5)

  a, b = utf8.find(S, B, 6)
  eq(a, nil)
  eq(b, nil)

  -- accepts zero
  a, b = utf8.find(S, A, 0)
  eq(a, 1)
  eq(b, 1)

  -- negative character-index init
  a, b = utf8.find(S, A, -5)
  eq(a, 1)
  eq(b, 1)

  a, b = utf8.find(S, A, -4)
  eq(a, nil)
  eq(b, nil)

  a, b = utf8.find(S, B, -1)
  eq(a, 5)
  eq(b, 5)

  a, b = utf8.find(S, EMOJI, -2)
  eq(a, 4)
  eq(b, 4)

  a, b = utf8.find(S, B, -6)
  eq(a, 5)
  eq(b, 5)

  -- anchored
  a, b = utf8.find(S, "^A")
  eq(a, 1)
  eq(b, 1)

  a, b = utf8.find(S, "^B")
  eq(a, nil)
  eq(b, nil)

  a, b = utf8.find(B..S, "B$")
  eq(a, 6)
  eq(b, 6)

  -- anchors at init position
  a, b = utf8.find(A..S, "^A", 2)
  eq(a, 2)
  eq(b, 2)

  a, b = utf8.find(S, ".", 2)
  eq(a, 2)
  eq(b, 2)

  -- plain mode
  a, b = utf8.find("a.b", ".", 1, true)
  eq(a, 2)
  eq(b, 2)

  a, b = utf8.find("a.b", ".", 1, false)
  eq(a, 1)
  eq(b, 1)

  -- a pattern without captures yields exactly the start and end positions
  eq(select("#", utf8.find(S, A)), 2)
  eq(select("#", utf8.find(S, "%a")), 2)
  eq(select("#", utf8.find(S, A, 1, true)), 2)

  -- captures follow the positions
  local s, e, c1, c2 = utf8.find(S, "(A)(é)")
  eq(s, 1)
  eq(e, 2)
  eq(c1, A)
  eq(c2, E_ACUTE)

  -- rejects malformed pattern
  raises(function() utf8.find("abc", "[") end)
end

---

-- utf8.match
do
  eq(utf8.match(S, "^(.)"), A)
  eq(utf8.match(S, "(🙂)"), EMOJI)
  eq(utf8.match(S, "(.)(.)(.)(.)(.)"), A)

  eq(utf8.match(S, "^B"), nil)
  eq(utf8.match(S, "Z"), nil)

  eq(utf8.match(S, "(.).", 1), "A")
  eq(utf8.match(S, "(.).", -2), "🙂")
  eq(utf8.match(S, "B", 6), nil)
  eq(utf8.match(S, "B", 0), B)

  local x, y = utf8.match("abcabc", "(abc)", 4)
  eq(x, "abc")
  eq(y, nil)

  -- captures containing multibyte characters
  local p, q = utf8.match("é中🙂", "^(..)(.)$")
  eq(p, "é中")
  eq(q, "🙂")

  -- match returns the whole match or the captures, never the positions.
  eq(select("#", utf8.match(S, A)), 1)
  eq(select("#", utf8.match(S, "(A)")), 1)

  local m1, m2, m3 = utf8.match("ab", "((a)(b))")
  eq(m1, "ab")
  eq(m2, "a")
  eq(m3, "b")

  -- position captures are integers
  p, q = utf8.match("abc", "()b()")
  eq(p, 2)
  eq(q, 3)

  -- an init past the last character yields nil
  eq(utf8.match(S, A, 12), nil)
  eq(utf8.match(S, "()", 12), nil)

  -- rejects malformed pattern
  raises(function() utf8.match("abc", "[") end)
  raises(function() utf8.match("abc", "a)") end)
end

---

-- utf8.gmatch
do
  local got = {}
  for c in utf8.gmatch(S, ".") do
    got[#got + 1] = c
  end
  assert_array(got, { A, E_ACUTE, CJK, EMOJI, B })

  got = {}
  for c in utf8.gmatch("one two three", "%w+") do
    got[#got + 1] = c
  end
  assert_array(got, { "one", "two", "three" })

  -- positional captures
  got = {}
  for p in utf8.gmatch(S, "()") do
    got[#got + 1] = p
  end
  assert_array(got, { 1, 2, 3, 4, 5, 6 })

  -- empty-match
  got = {}
  for x in utf8.gmatch("abc", "b?") do
    got[#got + 1] = x
  end
  assert_array(got, { "", "b", "" })

  -- an end anchor produces exactly one empty match
  local count = 0
  for _ in utf8.gmatch("abc", "$") do
    count = count + 1
  end
  eq(count, 1)

  -- rejects malformed pattern
  raises(function()
    for m in utf8.gmatch("abc", "[") do
      assert(m, nil)
    end
  end)
end

---

-- utf8.gsub
do
  local r, n = utf8.gsub(S, ".", "x")
  eq(r, "xxxxx")
  eq(n, 5)

  r, n = utf8.gsub(S, "(.)", "%1%1")
  eq(r, "AAéé中中🙂🙂BB")
  eq(n, 5)

  r, n = utf8.gsub(S, "é", "E")
  eq(r, "AE中🙂B")
  eq(n, 1)

  r, n = utf8.gsub(S, "🙂", "X")
  eq(r, "Aé中XB")
  eq(n, 1)

  r, n = utf8.gsub(S, ".", "x", 2)
  eq(r, "xx中🙂B")
  eq(n, 2)

  r, n = utf8.gsub(S, ".", "x", 0)
  eq(r, S)
  eq(n, 0)

  r, n = utf8.gsub(S, ".", "x", -1)
  eq(r, S)
  eq(n, 0)

  -- empty matches at beginning/end and between characters
  r, n = utf8.gsub("abc", "", "x")
  eq(r, "xaxbxcx")
  eq(n, 4)

  r, n = utf8.gsub("", "", "x")
  eq(r, "x")
  eq(n, 1)

  -- function replacement
  r, n = utf8.gsub(S, ".", function(c)
    return utf8.upper(c)
  end)
  eq(r, "AÉ中🙂B")
  eq(n, 5)

  -- table replacement
  r, n = utf8.gsub("aéa", "(.)", { a = "A", ["é"] = "E" })
  eq(r, "AEA")
  eq(n, 3)

  -- false/nil table replacements don't replace.
  r, n = utf8.gsub("abc", "(.)", { a = false, b = nil, c = "C" })
  eq(r, "abC")
  eq(n, 3)

  -- replacement escapes
  r, n = utf8.gsub("abc", "(.)", "%1%0")
  eq(r, "aabbcc")
  eq(n, 3)

  -- invalid capture indices
  raises(function() utf8.gsub("abc", "(.)", "%2") end)
  raises(function() utf8.gsub("abc", "(%1)", "x") end)

  -- anchors apply at most once
  r, n = utf8.gsub("abc", "^", "X")
  eq(r, "Xabc")
  eq(n, 1)

  r, n = utf8.gsub("abc", "$", "X")
  eq(r, "abcX")
  eq(n, 1)

  r, n = utf8.gsub("aaa", "^a", "X")
  eq(r, "Xaa")
  eq(n, 1)

  r, n = utf8.gsub("aaa", "a$", "X")
  eq(r, "aaX")
  eq(n, 1)

  -- numeric replacement results are stringified
  r, n = utf8.gsub("ab", ".", function()
    return 5
  end)
  eq(r, "55")
  eq(n, 2)

  -- booleans (other than false/nil) are rejected
  raises(function()
    utf8.gsub("ab", ".", function()
      return true
    end)
  end)
end

---

-- utf8.reverse
do
  eq(utf8.reverse(""), "")
  eq(utf8.reverse("abc"), "cba")
  eq(utf8.reverse(S), "B🙂中éA")
  eq(utf8.reverse("🙂é中"), "中é🙂")

  local bad = "A\xFFB"
  raises(function() utf8.reverse(bad) end)

  -- NUL and control bytes are preserved by reverse
  eq(utf8.reverse("\0\1\2\3"), "\3\2\1\0")

  -- lax mode must not error and must preserve the invalid byte
  local ok, r = pcall(function() return utf8.reverse(bad, true) end)
  check(ok)
  check(type(r) == "string")
  eq(#r, #bad)
end

---

-- utf8.lower / utf8.upper
do
  eq(utf8.lower("ABC"), "abc")
  eq(utf8.upper("abc"), "ABC")

  eq(utf8.lower("ÄÖÜ É À Ç"), "äöü é à ç")
  eq(utf8.upper("äöü é à ç"), "ÄÖÜ É À Ç")

  eq(utf8.lower("Straße"), "straße")
  eq(utf8.upper("straße"), "STRAßE")

  eq(utf8.lower("ǅ"), "ǆ")
  eq(utf8.upper("ǅ"), "Ǆ")

  eq(utf8.lower(0x41), 0x61)
  eq(utf8.upper(0x61), 0x41)

  eq(utf8.lower(0xC4), 0xE4)
  eq(utf8.upper(0xE4), 0xC4)

  -- NUL is preserved
  eq(utf8.lower("AB\0C"), "ab\0c")
  eq(utf8.upper("ab\0c"), "AB\0C")

  -- characters without mappings remain unchanged
  eq(utf8.lower(0x4E2D), 0x4E2D)
  eq(utf8.upper(0x4E2D), 0x4E2D)
  eq(utf8.lower(0x1F642), 0x1F642)
  eq(utf8.upper(0x1F642), 0x1F642)
end

---

-- utf8.title
do
  eq(utf8.title("hello world"), "HELLO WORLD")
  eq(utf8.title("ǆ"), "ǅ")
  eq(utf8.title("Ǆ"), "ǅ")

  eq(utf8.title(0x61), 0x41)
  eq(utf8.title(0xE9), 0xC9)

  -- characters without mappings remain unchanged
  eq(utf8.title(0x1F642), 0x1F642)
end

---

-- utf8.fold
do
  eq(utf8.fold("ABC"), "abc")
  eq(utf8.fold("Straße"), "straße")
  eq(utf8.fold("STRASSE"), "strasse")
  eq(utf8.fold("ÄÖÜ"), "äöü")

  eq(utf8.fold(0x41), 0x61)

  -- characters without mappings remain unchanged
  eq(utf8.fold(0x1F642), 0x1F642)
end

---

-- utf8.ncasecmp
do
  eq(utf8.ncasecmp("Hello", "hello"), 0)
  eq(utf8.ncasecmp("ABC", "abc"), 0)
  eq(utf8.ncasecmp("Straße", "STRAßE"), 0)

  eq(utf8.ncasecmp("abc", "abd"), -1)
  eq(utf8.ncasecmp("abd", "abc"), 1)

  eq(utf8.ncasecmp("abc", "ABCx"), -1)
  eq(utf8.ncasecmp("ABCx", "abc"), 1)

  eq(utf8.ncasecmp("", ""), 0)
  eq(utf8.ncasecmp("", "a"), -1)
  eq(utf8.ncasecmp("a", ""), 1)

  eq(utf8.ncasecmp("é", "É"), 0)
  eq(utf8.ncasecmp("é", "f"), 1)
  eq(utf8.ncasecmp("a", "é"), -1)

  -- multibyte case folding
  eq(utf8.ncasecmp("ä", "Ä"), 0)
  eq(utf8.ncasecmp("Ä", "ä"), 0)
  eq(utf8.ncasecmp("aB", "Ab"), 0)
  eq(utf8.ncasecmp("中", "中"), 0)
  eq(utf8.ncasecmp("🙂", "🙂"), 0)

  -- ordering is by code point after folding
  eq(utf8.ncasecmp("ä", "b"), 1)
  eq(utf8.ncasecmp("a", "ä"), -1)
end

---

-- utf8.offset
do
  -- n == 0 has special meaning: find the start of the character
  -- containing byte position i.
  eq(utf8.offset(S, 0, 1), 1)
  eq(utf8.offset(S, 0, 2), 2)
  eq(utf8.offset(S, 0, 3), 2)
  eq(utf8.offset(S, 0, 4), 4)
  eq(utf8.offset(S, 0, 5), 4)
  eq(utf8.offset(S, 0, 6), 4)
  eq(utf8.offset(S, 0, 7), 7)
  eq(utf8.offset(S, 0, 8), 7)
  eq(utf8.offset(S, 0, 9), 7)
  eq(utf8.offset(S, 0, 10), 7)
  eq(utf8.offset(S, 0, 11), 11)

  -- positive n from the beginning
  eq(utf8.offset(S, 1), 1)
  eq(utf8.offset(S, 2), 2)
  eq(utf8.offset(S, 3), 4)
  eq(utf8.offset(S, 4), 7)
  eq(utf8.offset(S, 5), 11)
  eq(utf8.offset(S, 6), 12)
  eq(utf8.offset(S, 7), nil)

  -- negative n from the end
  eq(utf8.offset(S, -1), 11)
  eq(utf8.offset(S, -2), 7)
  eq(utf8.offset(S, -3), 4)
  eq(utf8.offset(S, -4), 2)
  eq(utf8.offset(S, -5), 1)
  eq(utf8.offset(S, -6), nil)

  -- start position is a BYTE index
  eq(utf8.offset(S, 1, 1), 1)
  eq(utf8.offset(S, 1, 2), 2)
  eq(utf8.offset(S, 1, 4), 4)
  eq(utf8.offset(S, 1, 7), 7)
  eq(utf8.offset(S, 1, 11), 11)
  eq(utf8.offset(S, 1, 12), 12)

  eq(utf8.offset(S, 1, -1), 11)
  eq(utf8.offset(S, 4, -1), nil)
  eq(utf8.offset(S, 7, -1), nil)
  eq(utf8.offset(S, 11, -1), nil)

  -- zero byte position
  raises(function() utf8.offset(S, 1, 0) end)

  -- starting inside a multibyte character
  raises(function() utf8.offset(S, 1, 3) end)
  raises(function() utf8.offset(S, 1, 5) end)
  raises(function() utf8.offset(S, 1, 6) end)
  raises(function() utf8.offset(S, 1, 8) end)
  raises(function() utf8.offset(S, 1, 9) end)
  raises(function() utf8.offset(S, 1, 10) end)

  -- out of range byte positions
  raises(function() utf8.offset(S, 1, 13) end)

  -- offset returns the first and the last byte of the addressed character
  assert_array({ utf8.offset(S, 1) }, { 1, 1 })
  assert_array({ utf8.offset(S, 2) }, { 2, 3 })
  assert_array({ utf8.offset(S, 3) }, { 4, 6 })
  assert_array({ utf8.offset(S, 4) }, { 7, 10 })
  assert_array({ utf8.offset(S, 5) }, { 11, 11 })
  assert_array({ utf8.offset(S, 6) }, { 12, 12 })

  assert_array({ utf8.offset(S, -5) }, { 1, 1 })
  assert_array({ utf8.offset(S, 0) }, { 1, 1 })

  -- n == 0 reports the character containing a (possibly interior) byte
  assert_array({ utf8.offset(S, 0, 3) }, { 2, 3 })
  assert_array({ utf8.offset(S, 0, 5) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, 11) }, { 11, 11 })
end

---

-- utf8.codepoint
do
  eq(utf8.codepoint(S, 1), 0x41)
  eq(utf8.codepoint(S, 2), 0xE9)
  eq(utf8.codepoint(S, 4), 0x4E2D)
  eq(utf8.codepoint(S, 7), 0x1F642)
  eq(utf8.codepoint(S, 11), 0x42)

  local a, b, c, d, e = utf8.codepoint(S, 1, 11)
  eq(a, 0x41)
  eq(b, 0xE9)
  eq(c, 0x4E2D)
  eq(d, 0x1F642)
  eq(e, 0x42)

  a, b, c = utf8.codepoint(S, 2, 6)
  eq(a, 0xE9)
  eq(b, 0x4E2D)
  eq(c, nil)

  -- negative byte indices
  eq(utf8.codepoint(S, -1), 0x42)
  eq(utf8.codepoint(S, -5), 0x1F642)
  eq(utf8.codepoint(S, -8), 0x4E2D)
  eq(utf8.codepoint(S, -10), 0xE9)
  eq(utf8.codepoint(S, -11), 0x41)

  a, b = utf8.codepoint(S, -5, -1)
  eq(a, 0x1F642)
  eq(b, 0x42)

  -- zero is invalid as a byte position
  raises(function() utf8.codepoint(S, 0) end)
  raises(function() utf8.codepoint(S, 12) end)

  -- empty interval
  eq(utf8.codepoint(S, 5, 4), nil)

  -- invalid sequences
  raises(function() utf8.codepoint("\xFF") end)

  -- lax
  eq(utf8.codepoint(utf8.char(0xD800), 1, 1, true), 0xD800)
  eq(utf8.codepoint(utf8.char(0xFF), 1, 1, true), 0xFF)

  -- empty ranges return no code points
  eq(select("#", utf8.codepoint(S, 5, 4)), 0)

  -- positions that fall inside a sequence are errors
  raises(function() utf8.codepoint(S, 3) end)
  raises(function() utf8.codepoint(S, 6) end)
  raises(function() utf8.codepoint(S, 0, 3) end)

  -- lax mode accepts surrogates but still rejects overlong forms
  eq(utf8.codepoint(utf8.char(0xD800), 1, 1, true), 0xD800)
  raises(function() utf8.codepoint(utf8.char(0xD800), 1, 1) end)
  raises(function() utf8.codepoint("\xC0\x80", 1, 1, true) end)
  raises(function() utf8.codepoint("\xF0\x80\x80\x80", 1, 1, true) end)
end

---

-- utf8.codes
do
  local expected = {
    { 1,  0x41 },
    { 2,  0xE9 },
    { 4,  0x4E2D },
    { 7,  0x1F642 },
    { 11, 0x42 },
  }

  local got = collect_codes(S)
  assert_array(got, expected)

  local got2 = {}
  for p, cp in utf8.codes("é🙂") do
    got2[#got2 + 1] = { p, cp }
  end
  assert_array(got2, {
    { 1, 0xE9 },
    { 3, 0x1F642 },
  })

  raises(function() collect_codes(utf8.char(0xD800)) end)
  raises(function() collect_codes("\xFF") end)

  -- lax
  local got3 = collect_codes(utf8.char(0xD800), true)
  assert_array(got3, {
      { 1, 0xD800 }
  })

  -- reject out of range codepoint even with lax
  raises(function() collect_codes("\xFF", true) end)
end

---

-- utf8.escape
do
  eq(utf8.escape("abc"), "abc")
  eq(utf8.escape("%a"), "a")
  eq(utf8.escape("%%"), "%")
  eq(utf8.escape("%?"), "?")

  eq(utf8.escape("%65"), "A")
  eq(utf8.escape("%{65}"), "A")
  eq(utf8.escape("%u65"), "A")
  eq(utf8.escape("%u{65}"), "A")
  eq(utf8.escape("%x41"), "A")
  eq(utf8.escape("%x{41}"), "A")

  eq(utf8.escape("%20013"), "中")
  eq(utf8.escape("%{20013}"), "中")
  eq(utf8.escape("%u20013"), "中")
  eq(utf8.escape("%u{20013}"), "中")
  eq(utf8.escape("%x4E2D"), "中")
  eq(utf8.escape("%x{4E2D}"), "中")

  eq(utf8.escape("%%123"), "%123")
  eq(utf8.escape("%%u123"), "%u123")

  raises(function() utf8.escape("%") end)
  raises(function() utf8.escape("%{") end)
  raises(function() utf8.escape("%uA") end)
  raises(function() utf8.escape("%xG") end)
  raises(function() utf8.escape("%u{") end)
  raises(function() utf8.escape("%x{") end)
  raises(function() utf8.escape("%{65") end)
  raises(function() utf8.escape("%u{65") end)
  raises(function() utf8.escape("%x{41") end)

  -- the escape letter is case-insensitive: %u/%U are decimal, %x/%X are hex
  eq(utf8.escape("%U41"), ")")
  eq(utf8.escape("%X41"), A)
  eq(utf8.escape("%U{41}"), ")")
  eq(utf8.escape("%X{41}"), A)
  eq(utf8.escape("%u{0}"), "\0")
  eq(utf8.escape("%x{0}"), "\0")
  eq(utf8.escape("%{0}"), "\0")

  -- braced escapes accept only digits valid for their radix
  raises(function() utf8.escape("%{1a1}") end)
  raises(function() utf8.escape("%x{xyz}") end)
end

---

-- utf8.charpos
do
  local p, cp = utf8.charpos(S)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.charpos(S, 1)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.charpos(S, 2)
  eq(p, 2)
  eq(cp, 0xE9)

  p, cp = utf8.charpos(S, 3)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.charpos(S, 4)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.charpos(S, 5)
  eq(p, 11)
  eq(cp, 0x42)

  -- negative character positions
  p, cp = utf8.charpos(S, -1)
  eq(p, 11)
  eq(cp, 0x42)

  p, cp = utf8.charpos(S, -2)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.charpos(S, -5)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.charpos(S, 0)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.charpos(S, 6)
  eq(p, nil)
  eq(cp, nil)

  -- i is BYTE position, n is character count
  p, cp = utf8.charpos(S, 1, 1)
  eq(p, 2)
  eq(cp, 0xE9)

  p, cp = utf8.charpos(S, 1, 2)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.charpos(S, 1, 3)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.charpos(S, 1, 4)
  eq(p, 11)
  eq(cp, 0x42)

  p, cp = utf8.charpos(S, 1, 5)
  eq(p, nil)
  eq(cp, nil)

  -- negative n
  p, cp = utf8.charpos(S, 11, -1)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.charpos(S, 7, -1)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.charpos(S, 4, -1)
  eq(p, 2)
  eq(cp, 0xE9)

  -- zero n
  p, cp = utf8.charpos(S, 7, 0)
  eq(p, 7)
  eq(cp, 0x1F642)

  -- i inside a character
  p, cp = utf8.charpos(S, 3, 1)
  eq(p, 4)
  eq(cp, 0x4E2D)

  -- out of range
  p, cp = utf8.charpos(S, 12, 1)
  eq(p, nil)
  eq(cp, nil)

  p, cp = utf8.charpos(S, 1, 100)
  eq(p, nil)
  eq(cp, nil)
end

---

-- utf8.next
do
  local p, cp = utf8.next(S)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.next(S, 1)
  eq(p, 2)
  eq(cp, 0xE9)

  p, cp = utf8.next(S, 2)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.next(S, 4)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.next(S, 7)
  eq(p, 11)
  eq(cp, 0x42)

  p, cp = utf8.next(S, 11)
  eq(p, nil)
  eq(cp, nil)

  -- n.
  p, cp = utf8.next(S, 1, 1)
  eq(p, 2)
  eq(cp, 0xE9)

  p, cp = utf8.next(S, 1, 2)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.next(S, 1, 4)
  eq(p, 11)
  eq(cp, 0x42)

  -- negative n
  p, cp = utf8.next(S, 11, -1)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.next(S, 7, -1)
  eq(p, 4)
  eq(cp, 0x4E2D)

  p, cp = utf8.next(S, 4, -1)
  eq(p, 2)
  eq(cp, 0xE9)

  -- zero n
  p, cp = utf8.next(S, 7, 0)
  eq(p, 7)
  eq(cp, 0x1F642)

  -- inside multibyte sequence
  p, cp = utf8.next(S, 3)
  eq(p, 4)
  eq(cp, 0x4E2D)

  -- out of range
  p, cp = utf8.next(S, 12)
  eq(p, nil)
  eq(cp, nil)

  -- a zero offset starts at the first character
  p, cp = utf8.next(S, 0)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.next(S, 0, 1)
  eq(p, 1)
  eq(cp, 0x41)

  p, cp = utf8.next(S, 0, -1)
  eq(p, nil)
  eq(cp, nil)

  -- negative offsets walk backwards from the end
  p, cp = utf8.next(S, -5)
  eq(p, 11)
  eq(cp, 0x42)

  p, cp = utf8.next(S, -6)
  eq(p, 7)
  eq(cp, 0x1F642)

  p, cp = utf8.next(S, -1)
  eq(p, nil)
  eq(cp, nil)

  -- a negative n from a byte position past the end steps backwards
  p, cp = utf8.next(S, 12, -1)
  eq(p, 11)
  eq(cp, 0x42)

  -- iterator form
  local got = {}
  for pos, code in utf8.next, S do
    got[#got + 1] = { pos, code }
  end
  assert_array(got, {
    { 1,  0x41 },
    { 2,  0xE9 },
    { 4,  0x4E2D },
    { 7,  0x1F642 },
    { 11, 0x42 },
  })
end

---

-- utf8.insert
do
  eq(utf8.insert(S, "X"), S .. "X")
  eq(utf8.insert(S, 0, "X"), "Aé中🙂BX")
  eq(utf8.insert(S, 1, "X"), "XAé中🙂B")
  eq(utf8.insert(S, 2, "X"), "AXé中🙂B")
  eq(utf8.insert(S, 3, "X"), "AéX中🙂B")
  eq(utf8.insert(S, 4, "X"), "Aé中X🙂B")
  eq(utf8.insert(S, 5, "X"), "Aé中🙂XB")
  eq(utf8.insert(S, 6, "X"), "Aé中🙂BX")

  eq(utf8.insert(S, -1, "X"), "Aé中🙂XB")
  eq(utf8.insert(S, -2, "X"), "Aé中X🙂B")
  eq(utf8.insert(S, -5, "X"), "XAé中🙂B")

  -- out-of-range.
  raises(function() utf8.insert(S, 100, "X") end)
  raises(function() utf8.insert(S, -100, "X") end)

  eq(utf8.insert("", "X"), "X")
  eq(utf8.insert("", 1, "X"), "X")
  eq(utf8.insert("", 0, "X"), "X")

  eq(utf8.insert(S, 3, "🙂é"), "Aé🙂é中🙂B")

  -- a numeric insertion value is converted to its string form
  eq(utf8.insert("abc", 1, 5), "5abc")
end

---

-- utf8.remove
do
  eq(utf8.remove(S), "Aé中🙂")
  eq(utf8.remove(S, 1), "")
  eq(utf8.remove(S, 2), A)
  eq(utf8.remove(S, 3), "Aé")
  eq(utf8.remove(S, 5), "Aé中🙂")

  eq(utf8.remove(S, 1, 1), "é中🙂B")
  eq(utf8.remove(S, 2, 2), "A中🙂B")
  eq(utf8.remove(S, 3, 3), "Aé🙂B")
  eq(utf8.remove(S, 4, 4), "Aé中B")
  eq(utf8.remove(S, 5, 5), "Aé中🙂")

  eq(utf8.remove(S, -1, -1), "Aé中🙂")
  eq(utf8.remove(S, -2, -2), "Aé中B")
  eq(utf8.remove(S, -3, -1), "Aé")
  eq(utf8.remove(S, 2, -2), "AB")

  eq(utf8.remove(S, 0, 1), "é中🙂B")
  eq(utf8.remove(S, 6, 9), S)
  eq(utf8.remove(S, -100, 2), "中🙂B")
  eq(utf8.remove(S, 4, 2), S)

  eq(utf8.remove(""), "")
  eq(utf8.remove("", 1), "")
  eq(utf8.remove("", -1), "")

  -- remove deletes the selected character range.
  eq(utf8.remove(S, -1, 1), S)
  eq(utf8.remove(S, 2, 100), A)
  eq(utf8.remove(S, 6), S)
end

---

-- utf8.width
do
  eq(utf8.width("hello"), 5)
  eq(utf8.width("你好"), 4)
  eq(utf8.width("🙂"), 2)

  eq(utf8.width(0x41), 1)
  eq(utf8.width(0x4E2D), 2)
  eq(utf8.width(0x1F642), 2)

  -- Byte ranges.
  eq(utf8.width(S, 1, 1), 1)
  eq(utf8.width(S, 2, 3), 1)
  eq(utf8.width(S, 4, 6), 2)
  eq(utf8.width(S, 7, 10), 2)
  eq(utf8.width(S, 11, 11), 1)

  eq(utf8.width(S, 1, 11), 7)
  eq(utf8.width(S, 2, 10), 5)

  -- empty ranges / zero / negative byte indices
  eq(utf8.width(S, 1, 0), 0)
  eq(utf8.width(S, 5, 4), 0)
  eq(utf8.width(S, -1, -1), 1)
  eq(utf8.width(S, -5, -1), 3)
  eq(utf8.width(S, -11, -1), 7)

  -- ambiguous-width character
  eq(utf8.width("·"), 1)
  eq(utf8.width("·", 1, # "·", 2), 2)

  -- default width for control/unprintable
  eq(utf8.width("\1"), 1)
  eq(utf8.width("\1", 1, 1, 1, 3), 1)

  raises(function() utf8.width(S, 3, 3) end)
  raises(function() utf8.width(S, 5, 5) end)

  -- control characters keep a width of 1 regardless of default_width
  eq(utf8.width(0x00), 1)
  eq(utf8.width(0x00, 1, 5), 1)
  eq(utf8.width(0x7F), 1)
  eq(utf8.width(0x1F, 1, 5), 1)

  -- combining marks and format characters are zero-width unless overridden
  eq(utf8.width(0x0300), 0)
  eq(utf8.width(0x0300, 1, 5), 5)
  eq(utf8.width(0x200B), 0)
  eq(utf8.width(0x200B, 1, 5), 5)

  -- full-width forms
  eq(utf8.width(0x3000), 2)
  eq(utf8.width(0x4E2D), 2)

  raises(function() utf8.width(S, 2, 100) end)
  raises(function() utf8.width(S, 12, 12) end)
end

---

-- utf8.widthindex
do
  local i, off, w

  i, off, w = utf8.widthindex("abc", 1)
  eq(i, 1)
  eq(off, 1)
  eq(w, 1)

  i, off, w = utf8.widthindex("abc", 2)
  eq(i, 2)
  eq(off, 1)
  eq(w, 1)

  i, off, w = utf8.widthindex("abc", 3)
  eq(i, 3)
  eq(off, 1)
  eq(w, 1)

  i, off, w = utf8.widthindex("abc", 4)
  eq(i, 3)
  eq(off, nil)
  eq(w, nil)

  -- double-width characters
  i, off, w = utf8.widthindex("你a", 1)
  eq(i, 1)
  eq(off, 1)
  eq(w, 2)

  i, off, w = utf8.widthindex("你a", 2)
  eq(i, 1)
  eq(off, 2)
  eq(w, 2)

  i, off, w = utf8.widthindex("你a", 3)
  eq(i, 2)
  eq(off, 1)
  eq(w, 1)

  i, off, w = utf8.widthindex("你a", 4)
  eq(i, 2)
  eq(off, nil)
  eq(w, nil)

  -- zero/negative widths
  i, off, w = utf8.widthindex("abc", 0)
  eq(i, 1)
  eq(off, 0)
  eq(w, 1)

  i, off, w = utf8.widthindex("abc", -1)
  eq(i, 1)
  eq(off, -1)
  eq(w, 1)

  -- byte ranges
  i, off, w = utf8.widthindex(S, 1, 4, 6)
  eq(i, 1)
  eq(off, 1)
  eq(w, 2)

  i, off, w = utf8.widthindex(S, 2, 7, 10)
  eq(i, 1)
  eq(off, 2)
  eq(w, 2)

  -- ambiguous width
  i, off, w = utf8.widthindex("·a", 2, 1, 3, 2)
  eq(i, 1)
  eq(off, 2)
  eq(w, 2)

  raises(function() utf8.widthindex(S, 1, 3, 3) end)
end

---

-- utf8.widthlimit
do
  local p, remain = utf8.widthlimit("hello", 0)
  eq(p, 0)
  eq(remain, 0)

  p, remain = utf8.widthlimit("hello", 1)
  eq(p, 1)
  eq(remain, 0)

  p, remain = utf8.widthlimit("hello", 5)
  eq(p, 5)
  eq(remain, 0)

  p, remain = utf8.widthlimit("hello", 6)
  eq(p, 5)
  eq(remain, 1)

  p, remain = utf8.widthlimit("hello", 100)
  eq(p, 5)
  eq(remain, 95)

  -- a single double-width character
  p, remain = utf8.widthlimit("你", 1)
  eq(p, 0)
  eq(remain, 1)

  p, remain = utf8.widthlimit("你", 2)
  eq(p, 3)
  eq(remain, 0)

  -- negative limits work from the back
  p, remain = utf8.widthlimit("hello", -1)
  eq(p, 5)
  eq(remain, 0)

  p, remain = utf8.widthlimit("hello", -5)
  eq(p, 1)
  eq(remain, 0)

  p, remain = utf8.widthlimit("hello", -6)
  eq(p, 1)
  eq(remain, -1)

  p, remain = utf8.widthlimit("你a", 2)
  eq(p, 3)
  eq(remain, 0)

  p, remain = utf8.widthlimit("你a", 3)
  eq(p, 4)
  eq(remain, 0)

  -- byte ranges
  p, remain = utf8.widthlimit(S, 2, 4, 10)
  eq(p, 6)
  eq(remain, 0)

  p, remain = utf8.widthlimit(S, 3, 4, 10)
  eq(p, 6)
  eq(remain, 1)

  p, remain = utf8.widthlimit(S, -2, 1, 10)
  eq(p, 7)
  eq(remain, 0)

  -- empty range
  p, remain = utf8.widthlimit(S, 10, 5, 2)
  eq(p, 4)
  eq(remain, 10)

  p, remain = utf8.widthlimit(S, -10, 5, 2)
  eq(p, 3)
  eq(remain, -10)

  raises(function() utf8.widthlimit(S, 1, 3, 3) end)
end

---

-- utf8.isvalid
do
  check(utf8.isvalid(""))
  check(utf8.isvalid("ASCII"))
  check(utf8.isvalid(S))
  check(utf8.isvalid("é中🙂"))

  -- valid boundaries
  check(utf8.isvalid("\x7F"))
  check(utf8.isvalid("\xC2\x80"))
  check(utf8.isvalid("\xDF\xBF"))
  check(utf8.isvalid("\xE0\xA0\x80"))
  check(utf8.isvalid("\xEF\xBF\xBF"))
  check(utf8.isvalid("\xF0\x90\x80\x80"))
  check(utf8.isvalid("\xF4\x8F\xBF\xBF"))

  -- invalid boundaries
  check(not utf8.isvalid("\x80"))
  check(not utf8.isvalid("\xC0\x80"))
  check(not utf8.isvalid("\xE2\x28\xA1"))
  check(not utf8.isvalid("\xF0\x28\x8C\xBC"))
  check(not utf8.isvalid("\xED\xA0\x80"))
  check(not utf8.isvalid("\xF4\x90\x80\x80"))
end

---

-- utf8.clean
do
  local r, valid = utf8.clean("")
  eq(r, "")
  check(valid)

  -- cleaning valid input is identity
  r, valid = utf8.clean(S)
  eq(r, S)
  check(valid)

  r, valid = utf8.clean("A\xFFB")
  eq(r, "A�B")
  check(not valid)

  r, valid = utf8.clean("A\xFF\xFEB")
  eq(r, "A�B")
  check(not valid)

  r, valid = utf8.clean("\xFF\xFF")
  eq(r, "�")
  check(not valid)

  r, valid = utf8.clean("A\xFFB", "?")
  eq(r, "A?B")
  check(not valid)

  r, valid = utf8.clean("\xFF\xFEB", "")
  eq(r, "B")
  check(not valid)

  -- invalid byte followed by a valid multibyte sequence
  r, valid = utf8.clean("\xFFé\x80")
  eq(r, "�é�")
  check(not valid)

  -- consecutive invalid bytes are replaced with only one replacement character
  r, valid = utf8.clean("\xFF\x80\xFF")
  eq(r, "�")
  check(not valid)

  -- a replacement containing UTF-8 itself
  r, valid = utf8.clean("A\xFFB", "🙂")
  eq(r, "A🙂B")
  check(not valid)

  -- adjacent invalid bytes collapse into a single replacement
  r, valid = utf8.clean("\xFF\xFE")
  eq(r, "\u{FFFD}")
  check(not valid)

  r, valid = utf8.clean("\xFF\xFE", "?")
  eq(r, "?")
  check(not valid)

  r, valid = utf8.clean("A\xFF\xFEB", "?")
  eq(r, "A?B")
  check(not valid)
end

---

-- utf8.invalidoffset
do
  eq(utf8.invalidoffset(""), nil)
  eq(utf8.invalidoffset(S), nil)

  eq(utf8.invalidoffset("A\xFFB"), 2)
  eq(utf8.invalidoffset("A\xFF\xFEB"), 2)
  eq(utf8.invalidoffset("Aé\xFFB"), 4)

  -- start positions
  eq(utf8.invalidoffset("A\xFFB", 1), 2)
  eq(utf8.invalidoffset("A\xFFB", 2), 2)
  eq(utf8.invalidoffset("A\xFFB", 3), nil)

  -- negative positions
  eq(utf8.invalidoffset("A\xFFB", -1), nil)
  eq(utf8.invalidoffset("A\xFFB", -2), 2)
  eq(utf8.invalidoffset("A\xFFB", -3), 2)

  eq(utf8.invalidoffset("Aé\xFFB", -1), nil)
  eq(utf8.invalidoffset("Aé\xFFB", -2), 4)
  eq(utf8.invalidoffset("Aé\xFFB", -4), 4)

  -- zero and out of range.
  eq(utf8.invalidoffset("A\xFFB", 0), 2)
  eq(utf8.invalidoffset("A\xFFB", 10), nil)

  -- invalid-byte offsets: every possible start position
  local bad = "Aé\xFF🙂B"

  -- bytes:
  -- 1 A
  -- 2-3 é
  -- 4 FF
  -- 5-8 🙂
  -- 9 B
  local expected_bad = {
    [0] = 4,
    [1] = 4,
    [2] = 4,
    [3] = 3,
    [4] = 4,
    [5] = nil,
    [6] = 6,
    [7] = 7,
    [8] = 8,
    [9] = nil,
    [-1] = nil,
    [-2] = 8,
    [-3] = 7,
    [-4] = 6,
    [-5] = nil,
    [-6] = 4,
    [-7] = 3,
    [-8] = 4,
    [-9] = 4,
    [-10] = 4,
  }

  for i = -10, 9 do
    eq(
      utf8.invalidoffset(bad, i),
      expected_bad[i],
      "invalidoffset boundary " .. i
    )
  end
end

---
do
  check(utf8.isnfc(""))
  check(utf8.isnfc("ASCII"))
  check(utf8.isnfc("café"))
  check(utf8.isnfc("É"))
  check(utf8.isnfc("Å"))

  check(not utf8.isnfc("café"))
  check(not utf8.isnfc("A\u{030A}"))
  check(not utf8.isnfc("e\u{0301}"))

  -- already composed forms
  check(utf8.isnfc("\u{00E9}"))
  check(utf8.isnfc("\u{00C5}"))

  raises(function() utf8.isnfc("\xFF") end)
end

---

-- utf8.normalize_nfc
do
  local r, was = utf8.normalize_nfc("")
  eq(r, "")
  check(was)

  r, was = utf8.normalize_nfc("café")
  eq(r, "café")
  check(was)

  r, was = utf8.normalize_nfc("cafe\u{0301}")
  eq(r, "café")
  check(not was)

  r, was = utf8.normalize_nfc("A\u{030A}")
  eq(r, "Å")
  check(not was)

  r, was = utf8.normalize_nfc("e\u{0301}")
  eq(r, "é")
  check(not was)

  -- combining sequences
  r, was = utf8.normalize_nfc("a\u{0308}\u{0301}")
  eq(r, "ä\u{0301}")
  check(not was)

  -- function is idempotent
  local composed = utf8.normalize_nfc("Cafe\u{0301}")
  local normalized = composed
  local normalized2, already = utf8.normalize_nfc(normalized)
  eq(normalized2, normalized)
  check(already)

  raises(function() utf8.normalize_nfc("\xFF") end)
end

---

-- utf8.grapheme_indices
do
  local function graphemes(s, i, j)
    local r = {}
    local iter, state, var = utf8.grapheme_indices(s, i, j)
    while true do
      local a, b = iter(state, var)
      if a == nil then
        break
      end
      r[#r + 1] = { a, b }
      var = b
    end
    return r
  end

  -- simple ASCII: one grapheme per character
  assert_array(graphemes("abc"), {
    { 1, 1 },
    { 2, 2 },
    { 3, 3 },
  })

  -- combining mark stays with its base
  assert_array(graphemes("a\u{0301}b"), {
    { 1, 3 },
    { 4, 4 },
  })

  -- precomposed character is one grapheme
  assert_array(graphemes("áb"), {
    { 1, 2 },
    { 3, 3 },
  })

  -- emoji sequence / ZWJ and variation-selector cases
  local family = "👨‍👩‍👧"
  local gr = graphemes(family)
  eq(#gr, 1)

  local flag = "🇩🇪"
  gr = graphemes(flag)
  eq(#gr, 1)

  local skin = "👍🏽"
  gr = graphemes(skin)
  eq(#gr, 1)

  -- byte ranges
  assert_array(graphemes("a\u{0301}b", 1, 3), {
    { 1, 3 },
  })

  assert_array(graphemes("a\u{0301}b", 4, 4), {
    { 4, 4 },
  })

  -- invalid byte boundaries must not silently split a UTF-8 character
  raises(function() graphemes(S, 3) end)
  raises(function() graphemes(S, 5, 11) end)
  raises(function() graphemes(S, 12, 12) end)
end

---

-- utf8.charpattern
do
  -- charpattern matches exactly one UTF-8 character.
  local subj = "aé中🙂b"
  local count = 0
  for _ in subj:gmatch(utf8.charpattern) do
    count = count + 1
  end
  eq(count, utf8.len(subj))
end

---

-- cross-function invariants
do
  -- character index -> byte offset
  local expected_byte_positions = { 1, 2, 4, 7, 11 }
  for char_i = 1, 5 do
    local byte_i = expected_byte_positions[char_i]
    local p = utf8.charpos(S, char_i)
    eq(p, byte_i, "charpos position")

    local p2 = utf8.offset(S, char_i)
    eq(p2, byte_i, "offset position")

    local c = utf8.sub(S, char_i, char_i)
    eq(c, utf8.char(utf8.byte(S, char_i)), "character extraction")
  end

  -- char -> byte round trip
  for _, cp in ipairs({ 0x7F, 0x80, 0x7FF, 0x800, 0xFFFF, 0x10000, 0x10FFFF }) do
    eq(utf8.byte(utf8.char(cp)), cp, "char/byte round trip " .. cp)
  end

  -- every byte position returned by codes must agree with codepoint
  for pos, cp in utf8.codes(S) do
    eq(utf8.codepoint(S, pos), cp, "codes/codepoint agreement")
    local next_pos = utf8.offset(S, 2, pos)
    if next_pos then
      check(next_pos > pos)
    end
  end

  -- width must agree with the sum of individual codepoint widths.
  local total = 0
  for _, cp in utf8.codes(S) do
    total = total + utf8.width(cp)
  end
  eq(utf8.width(S), total, "width/codepoint agreement")
end
