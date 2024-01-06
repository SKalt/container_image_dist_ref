package main

import (
	"fmt"
	"os"
	"strings"

	// these are needed, else `digest.Parse()` will throw errors
	_ "crypto/sha256"
	_ "crypto/sha512"

	"github.com/distribution/reference"
)

// "github.com/distribution/reference"

func panicIf(err error) {
	if err != nil {
		panic(err)
	}
}

func parseValid(ref string, accumulator *strings.Builder) {
	result, err := reference.Parse(ref)
	if err != nil {
		panic(fmt.Sprintf("expected success, but produced %v for %s", err, ref))
	}
	if true { // <- useless block so I can visually align the output code
		_, err = accumulator.WriteString(fmt.Sprintf("- input:          \"%s\"\n", ref))
		panicIf(err)
		_, err = accumulator.WriteString(fmt.Sprintf("  result:         \"%s\"\n", result.String()))
		panicIf(err)
	}
	if named, ok := result.(reference.Named); ok {
		_, err = accumulator.WriteString(fmt.Sprintf("  name:           \"%s\"\n", named.Name()))
		panicIf(err)
		domain := reference.Domain(named)
		_, err = accumulator.WriteString(fmt.Sprintf("  domain:         \"%s\"\n", domain))
		panicIf(err)
		path := reference.Path(named)
		_, err = accumulator.WriteString(fmt.Sprintf("  path:           \"%s\"\n", path))
		panicIf(err)
	}
	if tagged, ok := result.(reference.Tagged); ok {
		_, err = accumulator.WriteString(fmt.Sprintf("  tag:            \"%s\"\n", tagged.Tag()))
		panicIf(err)
	} else {
		_, err = accumulator.WriteString(fmt.Sprintf("  tag:            null\n"))
		panicIf(err)
	}
	if digested, ok := result.(reference.Digested); ok {
		digest := digested.Digest()
		algorithm := digest.Algorithm().String()
		_, err = accumulator.WriteString(fmt.Sprintf("  digest_algo:    \"%s\"\n", algorithm))
		panicIf(err)
		digest.Encoded()
		_, err = accumulator.WriteString(fmt.Sprintf("  digest_encoded: \"%s\"\n", digest.Encoded()))
	} else {
		_, err = accumulator.WriteString(fmt.Sprintf("  digest_algo:    null\n"))
		panicIf(err)
		_, err = accumulator.WriteString(fmt.Sprintf("  digest_encoded: null\n"))
		panicIf(err)
	}
}

func parseInvalid(ref string, accumulator *strings.Builder) {
	result, errorOfInterest := reference.Parse(ref)
	if errorOfInterest == nil {
		panic(fmt.Sprintf("expected error, but produced %v for %s", result, ref))
	}
	_, err := accumulator.WriteString(fmt.Sprintf("- input: \"%s\"\n", ref))
	panicIf(err)
	_, err = accumulator.WriteString(fmt.Sprintf("  err:   \"%v\"\n", errorOfInterest))
	panicIf(err)
}

func main() {
	{ // handle valid inputs
		validInputs, err := os.ReadFile("./tests/fixtures/references/valid/inputs.txt")
		panicIf(err)
		validOutputs, err := os.Create("./tests/fixtures/references/valid/outputs.yaml")
		if err != nil {
			panic(err)
		}
		accumulator := strings.Builder{}
		for _, line := range strings.Split(string(validInputs), "\n") {
			if line == "" {
				continue
			}
			parseValid(line, &accumulator)
		}
		_, err = validOutputs.WriteString(accumulator.String())
		panicIf(err)
	}
	{
		// handle invalid inputs
		invalidInputs, err := os.ReadFile("./tests/fixtures/references/invalid/inputs.txt")
		panicIf(err)
		invalidOutputs, err := os.Create("./tests/fixtures/references/invalid/outputs.yaml")
		panicIf(err)
		accumulator := strings.Builder{}
		for _, line := range strings.Split(string(invalidInputs), "\n") {
			if line == "" {
				continue
			}
			parseInvalid(line, &accumulator)
		}
		_, err = invalidOutputs.WriteString(accumulator.String())
		panicIf(err)
	}
}
