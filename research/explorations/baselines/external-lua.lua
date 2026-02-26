-- tot: equivalent of ilo's tot(p, q, r) = p*q + p*q*r
local function tot(p, q, r)
    local s = p * q
    local t = s * r
    return s + t
end

local n = 10000
-- warmup
for i = 1, 1000 do
    tot(i, i+1, i+2)
end

local clock = os.clock
local start = clock()
local r = 0
for i = 1, n do
    r = tot(10, 20, 30)
end
local elapsed_s = clock() - start
local elapsed_ns = elapsed_s * 1e9
local per = elapsed_ns / n

io.write(string.format("result:     %d\n", r))
io.write(string.format("iterations: %d\n", n))
io.write(string.format("total:      %.2fms\n", elapsed_ns / 1e6))
io.write(string.format("per call:   %.0fns\n", per))
