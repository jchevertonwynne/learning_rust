package main

import (
	"fmt"
	"sync"
)

const THREADS = 1000
const REPEATS = 1000

func main() {
	x := make(map[int]int)
 
	var wg sync.WaitGroup
	for i := 0; i < THREADS; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()

			for repeat := 0; repeat < REPEATS; repeat++ {
				x[repeat] = repeat
			}
		} ()
	}
	wg.Wait()

	fmt.Printf("x = %d, expected %d\n", x, THREADS * REPEATS)
}
