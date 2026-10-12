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
-- 1  (-5)  A
-- 2  (-4)  é
-- 3  (-3)  中
-- 4  (-2)  🙂
-- 5  (-1)  B

-- byte indices:
-- 1  (-11)  A
-- 2  (-5)   é
-- 4  (-8)   中
-- 7  (-10)  🙂
-- 11 (-11)  B

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

-- utf8.offset
do
  -- requires at least one argument
  raises(function() utf8.offset(S) end)

  assert_array({ utf8.offset(S, -5) }, { 1, 1 })
  assert_array({ utf8.offset(S, -4) }, { 2, 3 })
  assert_array({ utf8.offset(S, -3) }, { 4, 6 })
  assert_array({ utf8.offset(S, -2) }, { 7, 10 })
  assert_array({ utf8.offset(S, -1) }, { 11, 11 })
  assert_array({ utf8.offset(S, 0) }, { 1, 1 })
  assert_array({ utf8.offset(S, 1) }, { 1, 1 })
  assert_array({ utf8.offset(S, 2) }, { 2, 3 })
  assert_array({ utf8.offset(S, 3) }, { 4, 6 })
  assert_array({ utf8.offset(S, 4) }, { 7, 10 })
  assert_array({ utf8.offset(S, 5) }, { 11, 11 })
  assert_array({ utf8.offset(S, 6) }, { 12, 12 })

  eq(utf8.offset(S, -100), nil)
  eq(utf8.offset(S, -6), nil)
  eq(utf8.offset(S, 7), nil)
  eq(utf8.offset(S, 100), nil)

  -- n == 0: find the start of the character containing byte position i
  assert_array({ utf8.offset(S, 0, -11) }, { 1, 1 })
  assert_array({ utf8.offset(S, 0, -10) }, { 2, 3 })
  assert_array({ utf8.offset(S, 0, -9) }, { 2, 3 })
  assert_array({ utf8.offset(S, 0, -8) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, -7) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, -6) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, -5) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, -4) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, -3) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, -2) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, -1) }, { 11, 11 })

  assert_array({ utf8.offset(S, 0, 1) }, { 1, 1 })
  assert_array({ utf8.offset(S, 0, 2) }, { 2, 3 })
  assert_array({ utf8.offset(S, 0, 3) }, { 2, 3 })
  assert_array({ utf8.offset(S, 0, 4) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, 5) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, 6) }, { 4, 6 })
  assert_array({ utf8.offset(S, 0, 7) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, 8) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, 9) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, 10) }, { 7, 10 })
  assert_array({ utf8.offset(S, 0, 11) }, { 11, 11 })
  assert_array({ utf8.offset(S, 0, 12) }, { 12, 12 })

  raises(function() utf8.offset(S, 0, 0) end)
  raises(function() utf8.offset(S, 0, -100) end)
  raises(function() utf8.offset(S, 0, -12) end)
  raises(function() utf8.offset(S, 0, 13) end)
  raises(function() utf8.offset(S, 0, 100) end)

  -- positive byte index
  assert_array({ utf8.offset(S, 1, 1) }, { 1, 1 })
  assert_array({ utf8.offset(S, 1, 2) }, { 2, 3 })
  assert_array({ utf8.offset(S, 1, 4) }, { 4, 6 })
  assert_array({ utf8.offset(S, 1, 7) }, { 7, 10 })
  assert_array({ utf8.offset(S, 1, 11) }, { 11, 11 })
  assert_array({ utf8.offset(S, 1, 12) }, { 12, 12 })

  assert_array({ utf8.offset(S, 1, -1) }, { 11, 11 })
  assert_array({ utf8.offset(S, 1, -5) }, { 7, 10 })
  assert_array({ utf8.offset(S, 1, -8) }, { 4, 6 })
  assert_array({ utf8.offset(S, 1, -10) }, { 2, 3 })
  assert_array({ utf8.offset(S, 1, -11) }, { 1, 1 })

  -- zero byte position
  raises(function() utf8.offset(S, 1, 0) end)

  -- starting inside a multibyte character
  raises(function() utf8.offset(S, 1, 3) end)
  raises(function() utf8.offset(S, 1, 5) end)
  raises(function() utf8.offset(S, 1, 6) end)
  raises(function() utf8.offset(S, 1, 8) end)
  raises(function() utf8.offset(S, 1, 9) end)
  raises(function() utf8.offset(S, 1, 10) end)
  raises(function() utf8.offset(S, 1, -9) end)
  raises(function() utf8.offset(S, 1, -7) end)
  raises(function() utf8.offset(S, 1, -6) end)
  raises(function() utf8.offset(S, 1, -4) end)
  raises(function() utf8.offset(S, 1, -3) end)
  raises(function() utf8.offset(S, 1, -2) end)

  raises(function() utf8.offset(S, 1, -12) end)
  raises(function() utf8.offset(S, 1, -100) end)
  raises(function() utf8.offset(S, 1, 13) end)
  raises(function() utf8.offset(S, 1, 100) end)

  -- mixed
  assert_array({ utf8.offset(S, 2, -11) }, { 2, 3 })
  assert_array({ utf8.offset(S, 2, -8) }, { 7, 10 })
  assert_array({ utf8.offset(S, 2, -5) }, { 11, 11 })
  assert_array({ utf8.offset(S, 3, -8) }, { 11, 11 })
  assert_array({ utf8.offset(S, 4, -10) }, { 11, 11 })
  assert_array({ utf8.offset(S, -4, -1) }, { 1, 1 })
  assert_array({ utf8.offset(S, -3, -1) }, { 2, 3 })
  assert_array({ utf8.offset(S, -2, -1) }, { 4, 6 })
  assert_array({ utf8.offset(S, -1, -1) }, { 7, 10 })
  assert_array({ utf8.offset(S, -2, -5) }, { 2, 3 })
  assert_array({ utf8.offset(S, -2, 4) }, { 1, 1 })
  assert_array({ utf8.offset(S, -3, 7) }, { 1, 1 })

  eq(utf8.offset(S, -5, -1), nil)
  eq(utf8.offset(S, 4, -1), nil)
  eq(utf8.offset(S, 7, -1), nil)
  eq(utf8.offset(S, 11, -1), nil)

  -- accepts invalid encodings
  assert_array({ utf8.offset("안\xFF녕", 3) }, { 5, 7 }) -- invalid leading byte
  assert_array({ utf8.offset("안\xC0\xAF녕", 3) }, { 6, 8 }) -- overlong
  assert_array({ utf8.offset("\x80\x80\x80", 0, 2) }, { 1, 3 }) -- starts with continuation bytes

  -- accepts surrogates and code points above 0x10FFFF
  assert_array({ utf8.offset("안\xED\xA0\x80녕", 3) }, { 7, 9 })
  assert_array({ utf8.offset("안\xF8\x88\x80\x80\x80녕", 3) }, { 9, 11 })
end

---

-- utf8.codepoint
do
  eq(utf8.codepoint(S, 1), 0x41)
  eq(utf8.codepoint(S, 2), 0xE9)
  eq(utf8.codepoint(S, 4), 0x4E2D)
  eq(utf8.codepoint(S, 7), 0x1F642)
  eq(utf8.codepoint(S, 11), 0x42)

  -- negative byte indices
  eq(utf8.codepoint(S, -1), 0x42)
  eq(utf8.codepoint(S, -5), 0x1F642)
  eq(utf8.codepoint(S, -8), 0x4E2D)
  eq(utf8.codepoint(S, -10), 0xE9)
  eq(utf8.codepoint(S, -11), 0x41)

  -- byte ranges
  assert_array({ utf8.codepoint(S, 1, 11) }, { 0x41, 0xE9, 0x4E2D, 0x1F642, 0x42 })
  assert_array({ utf8.codepoint(S, 2, 6) }, { 0xE9, 0x4E2D })
  assert_array({ utf8.codepoint(S, -5, -1) }, { 0x1F642, 0x42 })
  assert_array({ utf8.codepoint(S, -11, -5) }, { 0x41, 0xE9, 0x4E2D, 0x1F642 })
  assert_array({ utf8.codepoint(S, -5, 11) }, { 0x1F642, 0x42 })
  assert_array({ utf8.codepoint(S, 11, -1) }, { 0x42 })

  -- empty ranges
  eq(select("#", utf8.codepoint(S, 1, 0)), 0)
  eq(select("#", utf8.codepoint(S, 5, 4)), 0)

  -- zero is invalid as a byte position
  raises(function() utf8.codepoint(S, 0) end)

  -- positions that fall inside a sequence are errors
  raises(function() utf8.codepoint(S, 3) end)
  raises(function() utf8.codepoint(S, 6) end)
  raises(function() utf8.codepoint(S, 0, 3) end)

  -- surrogates and code points above 0x10FFFF are rejected
  raises(function() utf8.codepoint("\xED\xA0\x80", 1, 1) end)
  raises(function() utf8.codepoint("\xF8\x88\x80\x80\x80", 1, 1) end)

  -- lax mode accepts them
  eq(utf8.codepoint("\xED\xA0\x80", 1, 1, true), 0xD800)
  eq(utf8.codepoint("\xF8\x88\x80\x80\x80", 1, 1, true), 0x200000)

  -- lax mode rejects invalid encodings
  raises(function() utf8.codepoint("\xFF", 1, 1, true) end)
  raises(function() utf8.codepoint("\xC0\x80", 1, 1, true) end)
  raises(function() utf8.codepoint("\xF0\x80\x80\x80", 1, 1, true) end)
  raises(function() utf8.codepoint("\x80\x80\x80\x80", 1, 1, true) end)

  -- out-of-range byte positions are rejected
  raises(function() utf8.codepoint(S, -100) end)
  raises(function() utf8.codepoint(S, -12) end)
  raises(function() utf8.codepoint(S, 12) end)
  raises(function() utf8.codepoint(S, 13) end)
  raises(function() utf8.codepoint(S, 100) end)
  raises(function() utf8.codepoint(S, -1, 12) end)
end

---

-- utf8.codes
do
  assert_array(collect_codes(S), {
    { 1,  0x41 },
    { 2,  0xE9 },
    { 4,  0x4E2D },
    { 7,  0x1F642 },
    { 11, 0x42 },
  })

  assert_array(collect_codes("é🙂"), {
    { 1, 0xE9 },
    { 3, 0x1F642 },
  })

  -- surrogates and code points above 0x10FFFF are rejected
  raises(function() collect_codes("\xED\xA0\x80") end)
  raises(function() collect_codes("\xF8\x88\x80\x80\x80") end)

  -- lax mode accepts them
  assert_array(collect_codes("\xED\xA0\x80", true), { { 1, 0xD800 } })
  assert_array(collect_codes("\xF8\x88\x80\x80\x80", true), { { 1, 0x200000 } })

  -- lax mode rejects invalid encodings
  raises(function() collect_codes("\xFF", true) end)
  raises(function() collect_codes("\xC0\x80", true) end)
  raises(function() collect_codes("\xF0\x80\x80\x80", true) end)
  raises(function() collect_codes("\x80\x80\x80\x80", true) end)
end

---

-- utf8.byte
do
  eq(utf8.byte(S), 0x41)
  eq(utf8.byte(S, 1), 0x41)
  eq(utf8.byte(S, 2), 0xE9)
  eq(utf8.byte(S, 3), 0x4E2D)
  eq(utf8.byte(S, 4), 0x1F642)
  eq(utf8.byte(S, 5), 0x42)
  eq(utf8.byte(S, 6), nil)

  -- zero and negative character indices
  eq(utf8.byte(S, 0), nil)
  eq(utf8.byte(S, -1), 0x42)
  eq(utf8.byte(S, -2), 0x1F642)
  eq(utf8.byte(S, -3), 0x4E2D)
  eq(utf8.byte(S, -4), 0xE9)
  eq(utf8.byte(S, -5), 0x41)
  eq(utf8.byte(S, -6), nil)

  -- ranges
  assert_array({ utf8.byte(S, 1, 5) }, { 0x41, 0xE9, 0x4E2D, 0x1F642, 0x42 })
  assert_array({ utf8.byte(S, -5, -1) }, { 0x41, 0xE9, 0x4E2D, 0x1F642, 0x42 })
  assert_array({ utf8.byte(S, 0, 3) }, { 0x41, 0xE9, 0x4E2D })

  -- empty string
  eq(utf8.byte(EMPTY), nil)
  eq(utf8.byte(EMPTY, 1), nil)
  eq(utf8.byte(EMPTY, -1), nil)

  -- empty and out-of-range ranges return no values
  eq(select("#", utf8.byte(EMPTY)), 0)
  eq(select("#", utf8.byte(S, 3, 2)), 0)
  eq(select("#", utf8.byte(S, 100, 200)), 0)
  eq(select("#", utf8.byte(S, -100, -200)), 0)

  -- range endpoints clamp to the valid character range
  assert_array({ utf8.byte(S, -100, 100) }, { 0x41, 0xE9, 0x4E2D, 0x1F642, 0x42 })
  assert_array({ utf8.byte(S, 2, 100) }, { 0xE9, 0x4E2D, 0x1F642, 0x42 })
  assert_array({ utf8.byte(S, -2, 100) }, { 0x1F642, 0x42 })
  assert_array({ utf8.byte(S, 0, -1) }, { 0x41, 0xE9, 0x4E2D, 0x1F642, 0x42 })
  eq(select("#", utf8.byte(S, 100, 2)), 0)
  eq(select("#", utf8.byte(S, 100, -2)), 0)
  eq(select("#", utf8.byte(S, 6, 5)), 0)
  eq(select("#", utf8.byte(S, -6, -7)), 0)
  eq(select("#", utf8.byte(S, 7, 7)), 0)
  eq(select("#", utf8.byte(S, -7, -7)), 0)
  eq(select("#", utf8.byte(S, 1, 0)), 0)
  eq(select("#", utf8.byte(S, 0, 0)), 0)

  -- NUL bytes survive a byte -> char round trip.
  local nul = "\0x"
  eq(string.char(utf8.byte(nul, 1, -1)), nul)

  -- raises error on invalid utf8
  raises(function() utf8.byte("\xFF") end)     -- invalid leading byte
  raises(function() utf8.byte("\xC0\xAF") end) -- overlong
  -- but accepts surrogates and code points above 0x10FFFF
  eq(utf8.byte("\xED\xA0\x80"), 55296)
  eq(utf8.byte("\xF8\x88\x80\x80\x80"), 2097152)
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
  eq(utf8.len(S, 1, -1), 5)
  eq(utf8.len(S, -5, -5), 1)

  -- zero is invalid as a byte position
  raises(function() utf8.len(S, 0) end)
  raises(function() utf8.len(S, 0, 11) end)

  -- starting in the middle of a UTF-8 sequence
  assert_array({ utf8.len(S, 3, 3) }, { nil, 3 })
  assert_array({ utf8.len(S, 5, 10) }, { nil, 5 })
  assert_array({ utf8.len(S, -6, -1) }, { nil, 6 })

  -- end in the middle of a UTF-8 sequence
  eq(utf8.len(S, 1, 7), 4)
  eq(utf8.len(S, 1, 8), 4)
  eq(utf8.len(S, 1, 9), 4)
  eq(utf8.len(S, -11, -5), 4)

  -- invalid UTF-8/code points
  local bad1 = "\x80"
  local bad2 = "\xC0\x80"
  local bad3 = "\xE2\x28\xA1"
  local bad4 = "\xF0\x28\x8C\xBC"
  local bad5 = "\xED\xA0\x80"
  local bad6 = "\xF8\x88\x80\x80\x80"

  assert_array({ utf8.len(bad1) }, { nil, 1 })
  assert_array({ utf8.len(bad2) }, { nil, 1 })
  assert_array({ utf8.len(bad3) }, { nil, 1 })
  assert_array({ utf8.len(bad4) }, { nil, 1 })
  assert_array({ utf8.len(bad5) }, { nil, 1 })
  assert_array({ utf8.len(bad6) }, { nil, 1 })

  -- lax mode
  check(utf8.len(bad1, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad2, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad3, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad4, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len(bad5, nil, nil, true) ~= nil, "len should accept invalid byte in lax mode")
  check(utf8.len("a" .. bad6, nil, nil, true) == 2, "len should accept invalid byte in lax mode")

  -- an end beyond the last byte is rejected.
  raises(function() utf8.len(S, 2, 100) end)
  raises(function() utf8.len(S, 12, 12) end)

  -- every byte position as the initial index
  local len_expected = {
    [-11] = 5,
    [-10] = 4,
    [-8] = 3,
    [-5] = 2,
    [-1] = 1,
    [1] = 5,
    [2] = 4,
    [4] = 3,
    [7] = 2,
    [11] = 1,
    [12] = 0,
  }
  for i = -11, 12 do
    if len_expected[i] ~= nil then
      eq(utf8.len(S, i), len_expected[i], "utf8.len initial byte " .. i)
    end
  end

  -- positions that start in the middle of a sequence report the bad offset
  local len_mid = {
    [-9] = 3,
    [-7] = 5,
    [-6] = 6,
    [-4] = 8,
    [-3] = 9,
    [-2] = 10,
    [3] = 3,
    [5] = 5,
    [6] = 6,
    [8] = 8,
    [9] = 9,
    [10] = 10,
  }
  for i, pos in pairs(len_mid) do
    assert_array({ utf8.len(S, i) }, { nil, pos })
  end

  -- zero and positions beyond either end are rejected
  raises(function() utf8.len(S, 0) end)
  raises(function() utf8.len(S, -12) end)
  raises(function() utf8.len(S, -13) end)
  raises(function() utf8.len(S, 13) end)
  raises(function() utf8.len(S, 1, 12) end)
  raises(function() utf8.len(S, 1, 13) end)
  raises(function() utf8.len(S, -1, 12) end)
  raises(function() utf8.len(S, -12, -1) end)
end

---

-- utf8.sub
do
  eq(utf8.sub(S, 1), S)

  -- single index
  local sub_expected = {
    [-6] = EMPTY,
    [-5] = A,
    [-4] = E_ACUTE,
    [-3] = CJK,
    [-2] = EMOJI,
    [-1] = B,
    [0] = EMPTY,
    [1] = A,
    [2] = E_ACUTE,
    [3] = CJK,
    [4] = EMOJI,
    [5] = B,
    [6] = EMPTY,
  }

  for i = -6, 6 do
    eq(utf8.sub(S, i, i), sub_expected[i],
      "utf8.sub single-index boundary " .. i)
  end

  -- ranges
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

  -- two out-of-range indices clamp independently
  eq(utf8.sub(S, -6, -5), A)
  eq(utf8.sub(S, -100, -3), "Aé中")
  eq(utf8.sub(S, -3, 100), "中🙂B")
  eq(utf8.sub(S, 6, -6), EMPTY)
  eq(utf8.sub(S, 7, -7), EMPTY)
  eq(utf8.sub(S, 0, 1), A)

  eq(utf8.sub(EMPTY, 0, 0), EMPTY)
  eq(utf8.sub(EMPTY, -100, 100), EMPTY)
  eq(utf8.sub(EMPTY, 100, -100), EMPTY)

  -- accepts invalid utf8
  eq(utf8.sub("a\xFFb", 2), "\xFFb")                -- invalid leading byte
  eq(utf8.sub("a\xC0\xAFb", 2), "\xC0\xAFb")        -- overlong
  eq(utf8.sub("\x80\x80\x80b", 1), "\x80\x80\x80b") -- starts with continuation bytes
  -- accepts valid utf8 with surrogates and code points above 0x10FFFF
  eq(utf8.sub("a\xED\xA0\x80b", 2), "\xED\xA0\x80b")
  eq(utf8.sub("a\xF8\x88\x80\x80\x80b", 2), "\xF8\x88\x80\x80\x80b")
end

---

-- utf8.find
do
  local a, b = utf8.find(S, A)
  eq(a, 1)
  eq(b, 1)

  assert_array({ utf8.find(S, E_ACUTE) }, { 2, 2 })
  assert_array({ utf8.find(S, CJK) }, { 3, 3 })
  assert_array({ utf8.find(S, EMOJI) }, { 4, 4 })
  assert_array({ utf8.find(S, B) }, { 5, 5 })

  -- literal search
  assert_array({ utf8.find(S, "é中") }, { 2, 3 })

  -- empty pattern
  assert_array({ utf8.find(S, "") }, { 1, 0 })

  -- returns nil if not found
  eq(utf8.find(S, "not present"), nil)

  -- positive character-index init
  assert_array({ utf8.find(S, A, 1) }, { 1, 1 })
  assert_array({ utf8.find(S, A, 2) }, { nil, nil })
  assert_array({ utf8.find(S, B, 5) }, { 5, 5 })
  assert_array({ utf8.find(S, B, 6) }, { nil, nil })

  -- accepts zero
  assert_array({ utf8.find(S, A, 0) }, { 1, 1 })

  -- negative character-index init
  assert_array({ utf8.find(S, A, -5) }, { 1, 1 })
  assert_array({ utf8.find(S, A, -4) }, { nil, nil })
  assert_array({ utf8.find(S, B, -1) }, { 5, 5 })
  assert_array({ utf8.find(S, EMOJI, -2) }, { 4, 4 })
  assert_array({ utf8.find(S, B, -6) }, { 5, 5 })

  -- anchored
  assert_array({ utf8.find(S, "^A") }, { 1, 1 })
  assert_array({ utf8.find(S, "^B") }, { nil, nil })
  assert_array({ utf8.find(B .. S, "B$") }, { 6, 6 })

  -- anchors at init position
  assert_array({ utf8.find(A .. S, "^A", 2) }, { 2, 2 })

  -- plain mode
  assert_array({ utf8.find("a.b", ".", 1, true) }, { 2, 2 })
  assert_array({ utf8.find("a.b", ".", 1, false) }, { 1, 1 })

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

  -- out-of-range inits clamp towards the start for negative values
  assert_array({ utf8.find(S, A, 100) }, { nil, nil })
  assert_array({ utf8.find(S, A, -100) }, { 1, 1 })
  assert_array({ utf8.find(S, B, 100) }, { nil, nil })
  assert_array({ utf8.find(S, B, -100) }, { 5, 5 })

  -- plain search honors init as well
  assert_array({ utf8.find(S, B, -1, true) }, { 5, 5 })
  assert_array({ utf8.find(S, A, 2, true) }, { nil, nil })
  assert_array({ utf8.find(S, A, 0, true) }, { 1, 1 })

  -- the empty pattern reports the init and the character before it
  assert_array({ utf8.find(S, "", 6) }, { 6, 5 })
  assert_array({ utf8.find(S, "", 7) }, { nil, nil })
  assert_array({ utf8.find(S, "", -1) }, { 5, 4 })

  -- an anchor matches at the init position
  assert_array({ utf8.find(S, "^B", 5) }, { 5, 5 })
  assert_array({ utf8.find(S, "^B", 6) }, { nil, nil })
  assert_array({ utf8.find(S, "^A", -5) }, { 1, 1 })

  -- literal and plain searches clamp negative inits to the start
  local find_cjk = {
    [-7] = 3,
    [-6] = 3,
    [-5] = 3,
    [-4] = 3,
    [-3] = 3,
    [-2] = nil,
    [-1] = nil,
    [0] = 3,
    [1] = 3,
    [2] = 3,
    [3] = 3,
    [4] = nil,
    [5] = nil,
    [6] = nil,
    [7] = nil,
  }
  for init = -7, 7 do
    a, b = utf8.find(S, CJK, init)
    eq(a, find_cjk[init], "find literal init " .. init)

    a, b = utf8.find(S, CJK, init, true)
    eq(a, find_cjk[init], "find plain init " .. init)
  end

  -- pattern searches for every in-range init
  local find_dot = {
    [-5] = 1,
    [-4] = 2,
    [-3] = 3,
    [-2] = 4,
    [-1] = 5,
    [0] = 1,
    [1] = 1,
    [2] = 2,
    [3] = 3,
    [4] = 4,
    [5] = 5,
  }
  for init = -5, 5 do
    a, b = utf8.find(S, ".", init)
    eq(a, find_dot[init], "find pattern init " .. init)
    eq(b, find_dot[init], "find pattern end " .. init)
  end

  assert_array({ utf8.find(S, ".", 6) }, { nil, nil })
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

  assert_array({ utf8.match("abcabc", "(abc)", 4) }, { "abc", nil })

  -- captures containing multibyte characters
  assert_array({ utf8.match("é中🙂", "^(..)(.)$") }, { "é中", "🙂" })

  -- match returns the whole match or the captures, never the positions.
  eq(select("#", utf8.match(S, A)), 1)
  eq(select("#", utf8.match(S, "(A)")), 1)

  assert_array({ utf8.match("ab", "((a)(b))") }, { "ab", "a", "b" })

  -- position captures are integers
  assert_array({ utf8.match("abc", "()b()") }, { 2, 3 })

  -- an init past the last character yields nil
  eq(utf8.match(S, A, 12), nil)
  eq(utf8.match(S, "()", 12), nil)

  -- rejects malformed pattern
  raises(function() utf8.match("abc", "[") end)
  raises(function() utf8.match("abc", "a)") end)

  -- every negative init clamps to the first character
  eq(utf8.match(S, "(.)", -6), A)
  eq(utf8.match(S, "(.)", -5), A)
  eq(utf8.match(S, "(.)", -4), E_ACUTE)
  eq(utf8.match(S, "(.)", -3), CJK)
  eq(utf8.match(S, "(.)", -1), B)
  eq(utf8.match(S, "(.)", 100), nil)
  eq(utf8.match(S, "(.)", -100), A)

  -- a position capture at the end still matches
  eq(utf8.match(S, "()", 6), 6)
  eq(utf8.match(S, "()", 7), nil)

  -- an anchor matches at the init position
  eq(utf8.match(S, "^B", 5), B)
  eq(utf8.match(S, "^B", 1), nil)
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
  assert_array({ utf8.gsub(S, ".", "x") }, { "xxxxx", 5 })
  assert_array({ utf8.gsub(S, "(.)", "%1%1") }, { "AAéé中中🙂🙂BB", 5 })
  assert_array({ utf8.gsub(S, "é", "E") }, { "AE中🙂B", 1 })
  assert_array({ utf8.gsub(S, "🙂", "X") }, { "Aé中XB", 1 })
  assert_array({ utf8.gsub(S, ".", "x", 2) }, { "xx中🙂B", 2 })
  assert_array({ utf8.gsub(S, ".", "x", 0) }, { S, 0 })
  assert_array({ utf8.gsub(S, ".", "x", -1) }, { S, 0 })

  -- empty matches at beginning/end and between characters
  assert_array({ utf8.gsub("abc", "", "x") }, { "xaxbxcx", 4 })
  assert_array({ utf8.gsub("", "", "x") }, { "x", 1 })

  -- function replacement
  assert_array({ utf8.gsub(S, ".", function(c) return utf8.upper(c) end) }, { "AÉ中🙂B", 5 })

  -- table replacement
  assert_array({ utf8.gsub("aéa", "(.)", { a = "A", ["é"] = "E" }) }, { "AEA", 3 })

  -- false/nil table replacements don't replace.
  assert_array({ utf8.gsub("abc", "(.)", { a = false, b = nil, c = "C" }) }, { "abC", 3 })

  -- replacement escapes
  assert_array({ utf8.gsub("abc", "(.)", "%1%0") }, { "aabbcc", 3 })

  -- invalid capture indices
  raises(function() utf8.gsub("abc", "(.)", "%2") end)
  raises(function() utf8.gsub("abc", "(%1)", "x") end)

  -- anchors apply at most once
  assert_array({ utf8.gsub("abc", "^", "X") }, { "Xabc", 1 })
  assert_array({ utf8.gsub("abc", "$", "X") }, { "abcX", 1 })
  assert_array({ utf8.gsub("aaa", "^a", "X") }, { "Xaa", 1 })
  assert_array({ utf8.gsub("aaa", "a$", "X") }, { "aaX", 1 })

  -- numeric replacement results are stringified
  assert_array({ utf8.gsub("ab", ".", function() return 5 end) }, { "55", 2 })
  assert_array({ utf8.gsub("aé", "(.)", { a = 1, ["é"] = 2.2 }) }, { "12.2", 2 })

  -- booleans (other than false/nil) are rejected
  raises(function() utf8.gsub("ab", ".", function() return true end) end)

  -- replacement counts are clamped to the number of matches
  assert_array({ utf8.gsub(S, ".", "x", 4) }, { "xxxxB", 4 })
  assert_array({ utf8.gsub(S, ".", "x", 5) }, { "xxxxx", 5 })
  assert_array({ utf8.gsub(S, ".", "x", 100) }, { "xxxxx", 5 })
  assert_array({ utf8.gsub(S, ".", "x", -2) }, { S, 0 })
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

  -- raises error on invalid utf8
  raises(function() utf8.escape("\xFF") end)     -- invalid leading byte
  raises(function() utf8.escape("\xC0\xAF") end) -- overlong
  -- but accepts surrogates and code points above 0x10FFFF
  eq(utf8.escape("%x{41}\xED\xA0\x80"), "A\xED\xA0\x80")
  eq(utf8.escape("\xF8\x88\x80\x80\x80"), "\xF8\x88\x80\x80\x80")
end

---

-- utf8.charpos
do
  assert_array({ utf8.charpos(S) }, { 1, 0x41 })

  assert_array({ utf8.charpos(S, 1) }, { 1, 0x41 })
  assert_array({ utf8.charpos(S, 2) }, { 2, 0xE9 })
  assert_array({ utf8.charpos(S, 3) }, { 4, 0x4E2D })
  assert_array({ utf8.charpos(S, 4) }, { 7, 0x1F642 })
  assert_array({ utf8.charpos(S, 5) }, { 11, 0x42 })

  -- negative character positions
  assert_array({ utf8.charpos(S, -1) }, { 11, 0x42 })
  assert_array({ utf8.charpos(S, -2) }, { 7, 0x1F642 })
  assert_array({ utf8.charpos(S, -5) }, { 1, 0x41 })
  assert_array({ utf8.charpos(S, 0) }, { 1, 0x41 })
  assert_array({ utf8.charpos(S, 6) }, { nil, nil })

  -- i is byte position, n is character count
  assert_array({ utf8.charpos(S, 1, 1) }, { 2, 0xE9 })
  assert_array({ utf8.charpos(S, 1, 2) }, { 4, 0x4E2D })
  assert_array({ utf8.charpos(S, 1, 3) }, { 7, 0x1F642 })
  assert_array({ utf8.charpos(S, 1, 4) }, { 11, 0x42 })
  assert_array({ utf8.charpos(S, 1, 5) }, { nil, nil })

  -- negative n
  assert_array({ utf8.charpos(S, 11, -1) }, { 7, 0x1F642 })
  assert_array({ utf8.charpos(S, 7, -1) }, { 4, 0x4E2D })
  assert_array({ utf8.charpos(S, 4, -1) }, { 2, 0xE9 })

  -- zero n
  assert_array({ utf8.charpos(S, 7, 0) }, { 7, 0x1F642 })
  assert_array({ utf8.charpos(S, 0, 0) }, { 1, 0x41 })

  -- i == nil
  assert_array({ utf8.charpos(S, nil, 2) }, { 4, 0x4E2D })
  assert_array({ utf8.charpos(S, nil, 0) }, { 1, 0x41 })

  -- i inside a character
  assert_array({ utf8.charpos(S, 3, 1) }, { 4, 0x4E2D })

  -- out of range
  assert_array({ utf8.charpos(S, 12, 1) }, { nil, nil })
  assert_array({ utf8.charpos(S, 1, 100) }, { nil, nil })

  -- every negative character position, and positions beyond the end
  assert_array({ utf8.charpos(S, -3) }, { 4, 0x4E2D })
  assert_array({ utf8.charpos(S, -4) }, { 2, 0xE9 })
  assert_array({ utf8.charpos(S, -6) }, { nil, nil })
  assert_array({ utf8.charpos(S, 7) }, { nil, nil })

  -- out-of-range n with an explicit byte position
  assert_array({ utf8.charpos(S, 1, 6) }, { nil, nil })
  assert_array({ utf8.charpos(S, 1, -1) }, { nil, nil })
  assert_array({ utf8.charpos(S, 11, 1) }, { nil, nil })

  -- byte positions past the end still walk backwards
  assert_array({ utf8.charpos(S, 12, -1) }, { 11, 0x42 })
  assert_array({ utf8.charpos(S, 12, -5) }, { 1, 0x41 })
  assert_array({ utf8.charpos(S, 12, 0) }, { nil, nil })

  -- zero and negative byte positions clamp to the first byte
  assert_array({ utf8.charpos(S, 0, 1) }, { 2, 0xE9 })
  assert_array({ utf8.charpos(S, -11, 1) }, { 2, 0xE9 })

  -- accepts surrogates and code points above 0x10FFFF
  assert_array({ utf8.charpos("\xED\xA0\x80", 1) }, { 1, 0xD800 })
  assert_array({ utf8.charpos("\xF8\x88\x80\x80\x80", 1) }, { 1, 0x200000 })

  -- returns zero for invalid encodings
  assert_array({ utf8.charpos("\xFF", 1) }, { 1, 0 }) -- invalid leading byte
  assert_array({ utf8.charpos("\xC0\xAF", 1) }, { 1, 0 }) -- overlong
end

---

-- utf8.next
do
  assert_array({ utf8.next(S) }, { 1, 0x41 })


  assert_array({ utf8.next(S, -1) }, { nil, nil })
  assert_array({ utf8.next(S, -2) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -3) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -4) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -5) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -6) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, -7) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, -8) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, -9) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, -10) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, -11) }, { 2, 0xE9 })

  assert_array({ utf8.next(S, -12) }, { 1, 0x41 })
  assert_array({ utf8.next(S, -13) }, { 1, 0x41 })
  assert_array({ utf8.next(S, -100) }, { 1, 0x41 })

  assert_array({ utf8.next(S, 0) }, { 1, 0x41 })
  assert_array({ utf8.next(S, 1) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 2) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 3) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 4) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 5) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 6) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 7) }, { 11, 0x42 })
  assert_array({ utf8.next(S, 8) }, { 11, 0x42 })
  assert_array({ utf8.next(S, 9) }, { 11, 0x42 })
  assert_array({ utf8.next(S, 10) }, { 11, 0x42 })
  assert_array({ utf8.next(S, 11) }, { nil, nil })

  assert_array({ utf8.next(S, 12) }, { nil, nil })
  assert_array({ utf8.next(S, 13) }, { nil, nil })
  assert_array({ utf8.next(S, 100) }, { nil, nil })


  assert_array({ utf8.next(S, 0, 0) }, { 0, 0 })

  assert_array({ utf8.next(S, 1, -100) }, { nil, nil })
  assert_array({ utf8.next(S, 1, -1) }, { nil, nil })

  assert_array({ utf8.next(S, 1, 0) }, { 1, 0x41 })
  assert_array({ utf8.next(S, 1, 1) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 1, 2) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 1, 3) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 1, 4) }, { 11, 0x42 })

  assert_array({ utf8.next(S, 1, 5) }, { nil, nil })
  assert_array({ utf8.next(S, 1, 100) }, { nil, nil })

  -- negative n
  assert_array({ utf8.next(S, 11, -1) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 7, -1) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 4, -1) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 7, -2) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 7, -3) }, { 1, 0x41 })

  -- zero n
  assert_array({ utf8.next(S, 1, 0) }, { 1, 0x41 })
  assert_array({ utf8.next(S, 2, 0) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 3, 0) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 5, 0) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 7, 0) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 9, 0) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 11, 0) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -1, 0) }, { 11, 0x42 })
  assert_array({ utf8.next(S, -11, 0) }, { 1, 0x41 })

  assert_array({ utf8.next(S, -100, 0) }, { 0, 0 })
  assert_array({ utf8.next(S, -12, 0) }, { 0, 0 })
  assert_array({ utf8.next(S, 12, 0) }, { nil, nil })

  -- TODO: UB in original implementation? ignore for now
  --assert_array({ utf8.next(S, 100, 0) }, { 99, 0 })
  --assert_array({ utf8.next(S, 13, 0) }, { 13, 0 })
  --assert_array({ utf8.next(S, 15, 0) }, { 15, 0 })

  -- a zero offset starts at the first character
  assert_array({ utf8.next(S, 0, -100) }, { nil, nil })
  assert_array({ utf8.next(S, 0, -1) }, { nil, nil })
  assert_array({ utf8.next(S, 0) }, { 1, 0x41 })
  assert_array({ utf8.next(S, 0, 1) }, { 1, 0x41 })
  assert_array({ utf8.next(S, 0, 2) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 0, 3) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 0, 4) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 0, 5) }, { 11, 0x42 })
  assert_array({ utf8.next(S, 0, 6) }, { nil, nil })
  assert_array({ utf8.next(S, 0, 100) }, { nil, nil })

  -- a negative n from a byte position past the end steps backwards
  assert_array({ utf8.next(S, 12, -1) }, { 11, 0x42 })

  -- n steps from inside a multibyte sequence
  assert_array({ utf8.next(S, 3, -1) }, { 2, 0xE9 })
  assert_array({ utf8.next(S, 6, -1) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, 9, -2) }, { 4, 0x4E2D })
  assert_array({ utf8.next(S, -1, -1) }, { 7, 0x1F642 })
  assert_array({ utf8.next(S, 1, -1) }, { nil, nil })

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

  -- missing third argument
  raises(function() utf8.insert(S, 1) end)

  -- out-of-range.
  raises(function() utf8.insert(S, 100, "X") end)
  raises(function() utf8.insert(S, -100, "X") end)

  eq(utf8.insert("", "X"), "X")
  eq(utf8.insert("", 1, "X"), "X")
  eq(utf8.insert("", 0, "X"), "X")

  eq(utf8.insert(S, 3, "🙂é"), "Aé🙂é中🙂B")

  -- a numeric insertion value is converted to its string form
  eq(utf8.insert("abc", 1, 5), "5abc")
  eq(utf8.insert("abc", 1, 42), "42abc")
  eq(utf8.insert("abc", 1, 3.14), "3.14abc")

  -- one before the first character (and one past the end) is invalid
  raises(function() utf8.insert(S, -6, "X") end)
  raises(function() utf8.insert(S, -7, "X") end)
  raises(function() utf8.insert(S, 7, "X") end)
end

---

-- utf8.remove
do
  eq(utf8.remove(S), "Aé中🙂")

  -- single index removes from that character through the end
  local remove_from = {
    [-7] = EMPTY,
    [-6] = EMPTY,
    [-5] = EMPTY,
    [-4] = A,
    [-3] = "Aé",
    [-2] = "Aé中",
    [-1] = "Aé中🙂",
    [0] = EMPTY,
    [1] = EMPTY,
    [2] = A,
    [3] = "Aé",
    [4] = "Aé中",
    [5] = "Aé中🙂",
    [6] = S,
    [7] = S,
  }
  for i = -7, 7 do
    eq(utf8.remove(S, i), remove_from[i], "utf8.remove single index " .. i)
  end

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

  -- ranges touching or crossing the ends
  eq(utf8.remove(S, 5, 6), "Aé中🙂")
  eq(utf8.remove(S, 6, 5), S)
  eq(utf8.remove(S, 6, 6), S)
  eq(utf8.remove(S, -6, -6), S)
  eq(utf8.remove(S, -7, -7), S)
  eq(utf8.remove(S, 0, 0), S)
  eq(utf8.remove(S, 1, 0), S)
  eq(utf8.remove(S, 1, -1), EMPTY)
  eq(utf8.remove(S, -6, 6), EMPTY)
  eq(utf8.remove(S, 6, -6), S)
  eq(utf8.remove(S, -2, 2), S)
  eq(utf8.remove(S, -2, -1), "Aé中")
  eq(utf8.remove(S, -1, -2), S)
  eq(utf8.remove(S, 2, -100), S)
  eq(utf8.remove(S, -100, -100), S)
  eq(utf8.remove(S, 100, -3), S)
  eq(utf8.remove(S, -3, 100), "Aé")
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

  -- negative byte range and one past the end
  eq(utf8.width(S, -11, -5), 6)
  eq(utf8.width(S, -5, -5), 2)
  eq(utf8.width(S, -11, 11), 7)

  -- out-of-range byte positions are rejected
  raises(function() utf8.width(S, 0) end)
  raises(function() utf8.width(S, -12) end)
  raises(function() utf8.width(S, 13) end)
  raises(function() utf8.width(S, 1, 12) end)
  raises(function() utf8.width(S, 1, 13) end)
  raises(function() utf8.width(S, -12, -1) end)
end

---

-- utf8.widthindex
do
  assert_array({ utf8.widthindex("abc", 1) }, { 1, 1, 1 })
  assert_array({ utf8.widthindex("abc", 2) }, { 2, 1, 1 })
  assert_array({ utf8.widthindex("abc", 3) }, { 3, 1, 1 })
  assert_array({ utf8.widthindex("abc", 4) }, { 3, nil, nil })

  -- double-width characters
  assert_array({ utf8.widthindex("你a", 1) }, { 1, 1, 2 })
  assert_array({ utf8.widthindex("你a", 2) }, { 1, 2, 2 })
  assert_array({ utf8.widthindex("你a", 3) }, { 2, 1, 1 })
  assert_array({ utf8.widthindex("你a", 4) }, { 2, nil, nil })

  -- zero/negative widths
  assert_array({ utf8.widthindex("abc", 0) }, { 1, 0, 1 })
  assert_array({ utf8.widthindex("abc", -1) }, { 1, -1, 1 })

  -- byte ranges
  assert_array({ utf8.widthindex(S, 1, 4, 6) }, { 1, 1, 2 })
  assert_array({ utf8.widthindex(S, 2, 7, 10) }, { 1, 2, 2 })

  -- ambiguous width
  assert_array({ utf8.widthindex("·a", 2, 1, 3, 2) }, { 1, 2, 2 })

  raises(function() utf8.widthindex(S, 1, 3, 3) end)

  -- every display width of the fixture, including zero and negative
  local wi_expected = {
    [-3] = { 1, -3, 1 },
    [-2] = { 1, -2, 1 },
    [-1] = { 1, -1, 1 },
    [0] = { 1, 0, 1 },
    [1] = { 1, 1, 1 },
    [2] = { 2, 1, 1 },
    [3] = { 3, 1, 2 },
    [4] = { 3, 2, 2 },
    [5] = { 4, 1, 2 },
    [6] = { 4, 2, 2 },
    [7] = { 5, 1, 1 },
    [8] = { 5, nil, nil },
    [9] = { 5, nil, nil },
  }
  for width = -3, 9 do
    local idx, offset, win = utf8.widthindex(S, width)
    eq(idx, wi_expected[width][1], "widthindex idx " .. width)
    eq(offset, wi_expected[width][2], "widthindex offset " .. width)
    eq(win, wi_expected[width][3], "widthindex width " .. width)
  end

  -- a byte position one past the end yields an empty result
  assert_array({ utf8.widthindex(S, 1, 12) }, { 0, nil, nil })

  -- negative byte start and interior ranges
  assert_array({ utf8.widthindex(S, 1, -11, -1) }, { 1, 1, 1 })

  assert_array({ utf8.widthindex(S, 3, 4, 10) }, { 2, 1, 2 })

  assert_array({ utf8.widthindex(S, 0, 4, 10) }, { 1, 0, 2 })

  -- out-of-range byte positions are rejected
  raises(function() utf8.widthindex(S, 1, 0) end)
  raises(function() utf8.widthindex(S, 1, 13) end)
  raises(function() utf8.widthindex(S, 1, -12) end)
  raises(function() utf8.widthindex(S, 1, 1, 12) end)
end

---

-- utf8.widthlimit
do
  assert_array({ utf8.widthlimit("hello", 0) }, { 0, 0 })
  assert_array({ utf8.widthlimit("hello", 1) }, { 1, 0 })
  assert_array({ utf8.widthlimit("hello", 5) }, { 5, 0 })
  assert_array({ utf8.widthlimit("hello", 6) }, { 5, 1 })
  assert_array({ utf8.widthlimit("hello", 100) }, { 5, 95 })

  -- a single double-width character
  assert_array({ utf8.widthlimit("你", 1) }, { 0, 1 })
  assert_array({ utf8.widthlimit("你", 2) }, { 3, 0 })

  -- negative limits work from the back
  assert_array({ utf8.widthlimit("hello", -1) }, { 5, 0 })
  assert_array({ utf8.widthlimit("hello", -5) }, { 1, 0 })
  assert_array({ utf8.widthlimit("hello", -6) }, { 1, -1 })
  assert_array({ utf8.widthlimit("你a", 2) }, { 3, 0 })
  assert_array({ utf8.widthlimit("你a", 3) }, { 4, 0 })

  -- byte ranges
  assert_array({ utf8.widthlimit(S, 2, 4, 10) }, { 6, 0 })
  assert_array({ utf8.widthlimit(S, 3, 4, 10) }, { 6, 1 })
  assert_array({ utf8.widthlimit(S, -2, 1, 10) }, { 7, 0 })

  -- empty range
  assert_array({ utf8.widthlimit(S, 10, 5, 2) }, { 4, 10 })
  assert_array({ utf8.widthlimit(S, -10, 5, 2) }, { 3, -10 })
  raises(function() utf8.widthlimit(S, 1, 3, 3) end)

  -- every limit for the whole fixture
  local wl_expected = {
    [-12] = { 1, -5 },
    [-11] = { 1, -4 },
    [-10] = { 1, -3 },
    [-9] = { 1, -2 },
    [-8] = { 1, -1 },
    [-7] = { 1, 0 },
    [-6] = { 2, 0 },
    [-5] = { 4, 0 },
    [-4] = { 7, -1 },
    [-3] = { 7, 0 },
    [-2] = { 11, -1 },
    [-1] = { 11, 0 },
    [0] = { 0, 0 },
    [1] = { 1, 0 },
    [2] = { 3, 0 },
    [3] = { 3, 1 },
    [4] = { 6, 0 },
    [5] = { 6, 1 },
    [6] = { 10, 0 },
    [7] = { 11, 0 },
    [8] = { 11, 1 },
    [9] = { 11, 2 },
    [10] = { 11, 3 },
    [11] = { 11, 4 },
    [12] = { 11, 5 },
  }
  for limit = -12, 12 do
    assert_array({ utf8.widthlimit(S, limit) }, wl_expected[limit])
  end

  -- interior ranges
  assert_array({ utf8.widthlimit(S, 1, 5, 4) }, { 4, 1 })
  assert_array({ utf8.widthlimit(S, 1, 4, 2) }, { 3, 1 })
  assert_array({ utf8.widthlimit(S, -2, 4, 10) }, { 7, 0 })

  -- out-of-range byte positions are rejected
  raises(function() utf8.widthlimit(S, 1, 0) end)
  raises(function() utf8.widthlimit(S, 1, 13) end)
  raises(function() utf8.widthlimit(S, 1, -12) end)
  raises(function() utf8.widthlimit(S, 1, 1, 12) end)
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

  -- starting beyond either end finds nothing
  eq(utf8.invalidoffset(bad, 10), nil)
  eq(utf8.invalidoffset(bad, 12), nil)
  eq(utf8.invalidoffset(bad, -12), 4)

  -- the empty string is always valid, whatever the start index
  eq(utf8.invalidoffset("", 0), nil)
  eq(utf8.invalidoffset("", 1), nil)
  eq(utf8.invalidoffset("", -1), nil)
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

  -- a clampable initial byte position starts the scan there
  assert_array(graphemes(S, -5), {
    { 7,  10 },
    { 11, 11 },
  })

  assert_array(graphemes(S, -11), {
    { 1,  1 },
    { 2,  3 },
    { 4,  6 },
    { 7,  10 },
    { 11, 11 },
  })

  -- byte positions at the very end produce no clusters
  assert_array(graphemes(S, 12), {})
  assert_array(graphemes(S, 13), {})

  -- negative byte ranges
  assert_array(graphemes(S, -5, -1), {
    { 7,  10 },
    { 11, 11 },
  })

  assert_array(graphemes(S, -11, -5), {
    { 1, 1 },
    { 2, 3 },
    { 4, 6 },
    { 7, 10 },
  })

  -- single-character byte ranges
  assert_array(graphemes(S, 1, 1), { { 1, 1 } })
  assert_array(graphemes(S, 2, 2), { { 2, 3 } })
  assert_array(graphemes(S, 2, 3), { { 2, 3 } })
  assert_array(graphemes(S, 11, 11), { { 11, 11 } })
  assert_array(graphemes(S, -5, -5), { { 7, 10 } })
  assert_array(graphemes(S, -1, -1), { { 11, 11 } })

  -- out-of-range and interior byte positions
  raises(function() graphemes(S, 0) end)
  raises(function() graphemes(S, -12) end)
  raises(function() graphemes(S, -13) end)
  raises(function() graphemes(S, 3, 3) end)
  raises(function() graphemes(S, 5, 5) end)
  raises(function() graphemes(S, 6, 7) end)
  raises(function() graphemes(S, -6, -1) end)
  raises(function() graphemes(S, 1, 12) end)
  raises(function() graphemes(S, 1, 13) end)
  raises(function() graphemes(S, -12, -1) end)
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
