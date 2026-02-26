def tot(p, q, r)
  s = p * q
  t = s * r
  s + t
end

n = 10000
1000.times { |i| tot(i, i+1, i+2) }

start = Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)
r = 0
n.times { r = tot(10, 20, 30) }
elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond) - start
per = elapsed / n

puts "result:     #{r}"
puts "iterations: #{n}"
puts "total:      #{'%.2f' % (elapsed / 1e6)}ms"
puts "per call:   #{per}ns"
