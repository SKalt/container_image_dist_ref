package main

import (
	"strings"
	"testing"

	"github.com/distribution/reference"

	// these are needed, else `digest.Parse()` will throw errors
	_ "crypto/sha256"
	_ "crypto/sha512"
	_ "embed"
)

//go:embed inputs.txt
var rawInputs string

func BenchmarkOracleEntireTestSuite(b *testing.B) {
	var inputs = strings.Split(rawInputs, "\n")
	var filtered = make([]string, 0, len(inputs))
	for _, ref := range inputs {
		if ref != "" {
			filtered = append(filtered, ref)
		}
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for _, ref := range filtered {
			reference.Parse(ref)
		}
	}
}

func BenchmarkJustIteration(b *testing.B) {
	var inputs = strings.Split(rawInputs, "\n")
	var filtered = make([]string, 0, len(inputs))
	for _, ref := range inputs {
		if ref != "" {
			filtered = append(filtered, ref)
		}
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for _, _ = range filtered {
		}
	}
}
