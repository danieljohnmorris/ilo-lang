package main

import (
	"fmt"
	"time"
)

//go:noinline
func tot(p, q, r float64) float64 {
	s := p * q
	t := s * r
	return s + t
}

func main() {
	n := 10000
	for i := 0; i < 1000; i++ {
		tot(float64(i), float64(i+1), float64(i+2))
	}

	start := time.Now()
	var r float64
	for i := 0; i < n; i++ {
		r = tot(10, 20, 30)
	}
	elapsed := time.Since(start)
	per := elapsed.Nanoseconds() / int64(n)

	fmt.Printf("result:     %.0f\n", r)
	fmt.Printf("iterations: %d\n", n)
	fmt.Printf("total:      %.2fms\n", float64(elapsed.Nanoseconds())/1e6)
	fmt.Printf("per call:   %dns\n", per)
}
