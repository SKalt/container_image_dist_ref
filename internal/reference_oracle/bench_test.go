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
var inputs = strings.Split(rawInputs, "\n")

func BenchmarkOracleEntireTestSuite(b *testing.B) {
	filtered := make([]string, 0, len(inputs))
	for _, ref := range inputs {
		if ref != "" {
			filtered = append(filtered, ref)
		}
	}
	for i := 0; i < b.N; i++ {
		for _, ref := range filtered {
			reference.Parse(ref)
		}
	}
}

// TODO: use wazero to benchmark the wasm version of the rust library
