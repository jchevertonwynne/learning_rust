package main

import (
	"fmt"
	"os"
	"strconv"
)

func main() {
	filename := "yolo"
	if len(os.Args) > 1 {
		filename = os.Args[1]
	}

	i, err := intFromFile(filename)
	if err != nil {
		fmt.Printf("failed to read int: %s\n", err.Error())
		os.Exit(1)
	}

	fmt.Printf("int is %d\n", i)
}

func intFromFile(filename string) (int, error) {
	b, err := os.ReadFile(filename)
	if err != nil {
		return 0, err
	}
	
	i, err := strconv.Atoi(string(b))
	if err != nil {
		return 0, err
	}
	
	return i, nil
}