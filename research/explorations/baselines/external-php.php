<?php
function tot($p, $q, $r) {
    $s = $p * $q;
    $t = $s * $r;
    return $s + $t;
}

$n = 10000;
for ($i = 0; $i < 1000; $i++) tot($i, $i+1, $i+2);

$start = hrtime(true);
$r = 0;
for ($i = 0; $i < $n; $i++) $r = tot(10, 20, 30);
$elapsed = hrtime(true) - $start;
$per = intdiv($elapsed, $n);

echo "result:     $r\n";
echo "iterations: $n\n";
echo sprintf("total:      %.2fms\n", $elapsed / 1e6);
echo "per call:   {$per}ns\n";
