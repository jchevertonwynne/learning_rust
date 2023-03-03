package main

import (
	"fmt"
	"strings"
)

func main() {
	fullString := "hello world"
	subStrings := []string {
		"world",
		"abacus",
	}

	for _, subString := range subStrings {
		index := strings.Index(fullString, subString)
		if index == -1 {
			continue
		}
		
		fmt.Printf("substring %s was found in %s at index %d\n", subString, fullString, index)
		
		recreatedSubstring := fullString[index:index + len(subString)]
		if subString != recreatedSubstring {
			panic("recreated substring did not match original substring")
		}
	}
}