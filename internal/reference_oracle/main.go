package main

import (
	"fmt"
	"io"
	"os"
	"strings"

	// these are needed, else `digest.Parse()` will throw errors
	_ "crypto/sha256"
	_ "crypto/sha512"

	"github.com/distribution/reference"
	"github.com/opencontainers/go-digest"
)

func panicIf(err error) {
	if err != nil {
		panic(err)
	}
}
func mustWrite(writer io.StringWriter, s string) {
	_, err := writer.WriteString(s)
	panicIf(err)
}

type parseResult struct {
	input, name, domain, path, tag, digestAlgo, digestEncoded, err string
}

func (result parseResult) row() string {
	return strings.Join([]string{
		result.input,
		result.name,
		result.domain,
		result.path,
		result.tag,
		result.digestAlgo,
		result.digestEncoded,
		result.err,
	}, "\t") + "\n"
}

func parse(ref string) (result parseResult) {
	result.input = ref
	parsed, err := reference.Parse(ref)
	if err != nil {
		result.err = err.Error()
		switch err {
		case reference.ErrReferenceInvalidFormat:
		case reference.ErrTagInvalidFormat:
		case reference.ErrDigestInvalidFormat:
		case reference.ErrNameContainsUppercase:
		case reference.ErrNameEmpty:
		case reference.ErrNameTooLong:
		case reference.ErrNameNotCanonical:

		case digest.ErrDigestInvalidFormat:
		case digest.ErrDigestInvalidLength:
		case digest.ErrDigestUnsupported:
			break
		default:
			panic(fmt.Sprintf("unexpected error: %v", err))
		}
		return
	} else {
		if named, ok := parsed.(reference.Named); ok {
			result.name = named.Name()
			result.domain = reference.Domain(named)
			result.path = reference.Path(named)
		}
		if tagged, ok := parsed.(reference.Tagged); ok {
			result.tag = tagged.Tag()
		}
		if digested, ok := parsed.(reference.Digested); ok {
			digest := digested.Digest()
			result.digestAlgo = digest.Algorithm().String()
			result.digestEncoded = digest.Encoded()
		}
		return
	}
}

func parseFileLines(inputs string, output io.StringWriter) {
	for _, line := range strings.Split(string(inputs), "\n") {
		if line == "" {
			continue
		}
		mustWrite(output, parse(line).row())
	}
}

func main() {
	validInputs, err := os.ReadFile("./tests/fixtures/references/valid/inputs.txt")
	panicIf(err)
	validOutputs, err := os.Create("./tests/fixtures/references/outputs.tsv")
	panicIf(err)
	accumulator := strings.Builder{}
	header := strings.Join([]string{
		"input",
		"name",
		"domain",
		"path",
		"tag",
		"digest_algo",
		"digest_encoded",
		"err",
	}, "\t") + "\n"
	mustWrite(&accumulator, header)
	parseFileLines(string(validInputs), &accumulator)

	// handle invalid inputs
	invalidInputs, err := os.ReadFile("./tests/fixtures/references/invalid/inputs.txt")
	panicIf(err)
	parseFileLines(string(invalidInputs), &accumulator)

	// flush the accumulator to the output file
	mustWrite(validOutputs, accumulator.String())
}
