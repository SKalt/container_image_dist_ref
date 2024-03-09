package reference_oracle

import (
	"fmt"
	"io"
	"os"
	"regexp"
	"strings"

	// these are needed, else `digest.Parse()` will throw errors
	_ "crypto/sha256"
	_ "crypto/sha512"

	"github.com/distribution/reference"
	"github.com/opencontainers/go-digest"
)

func PanicIf(err error) {
	if err != nil {
		panic(err)
	}
}
func MustWrite(writer io.StringWriter, s string) {
	_, err := writer.WriteString(s)
	PanicIf(err)
}

type ParseResult struct {
	Input, Name, Domain, Path, Tag, DigestAlgo, DigestEncoded, Err string
}

func Ipv6ExpectedFailure(errName string) bool {
	return strings.HasPrefix(errName, "Ipv6") && (strings.HasPrefix(errName, "Ipv6TooLong") ||
		strings.HasPrefix(errName, "Ipv6BadColon") ||
		strings.HasPrefix(errName, "Ipv6TooManyHexDigits") ||
		strings.HasPrefix(errName, "Ipv6TooManyGroups") ||
		strings.HasPrefix(errName, "Ipv6TooFewGroups"))
}

func (result ParseResult) row() string {
	return strings.Join([]string{
		result.Input,
		result.Name,
		result.Domain,
		result.Path,
		result.Tag,
		result.DigestAlgo,
		result.DigestEncoded,
		result.Err,
	}, "\t") + "\n"
}

func parseRef(ref string) (result ParseResult) {
	result.Input = ref
	parsed, err := reference.Parse(ref)
	if err != nil {
		result.Err = err.Error()
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
			result.Name = named.Name()
			result.Domain = reference.Domain(named)
			result.Path = reference.Path(named)
		}
		if tagged, ok := parsed.(reference.Tagged); ok {
			result.Tag = tagged.Tag()
		}
		if digested, ok := parsed.(reference.Digested); ok {
			digest := digested.Digest()
			result.DigestAlgo = digest.Algorithm().String()
			result.DigestEncoded = digest.Encoded()
		}
		return
	}
}

func ParseCanonical(ref string) (result ParseResult) {
	result.Input = ref
	parsed, err := reference.ParseNamed(ref)
	if err != nil {
		result.Err = err.Error()
		return
	} else {
		if canonical, ok := parsed.(reference.Canonical); ok {
			result.Name = canonical.Name()
			result.Domain = reference.Domain(canonical)
			result.Path = reference.Path(canonical)
			if tagged, ok := canonical.(reference.Tagged); ok {
				result.Tag = tagged.Tag()
			}
			digest := canonical.Digest()
			result.DigestAlgo = digest.Algorithm().String()
			result.DigestEncoded = digest.Encoded()
		} else {
			result.Err = "not canonical"
		}
		return
	}
}

func parseFileLines(inputs string, output io.StringWriter) {
	for _, line := range strings.Split(string(inputs), "\n") {
		if line == "" {
			continue
		}
		MustWrite(output, parseRef(line).row())
	}
}

func unescape(s string) string {
	s = strings.ReplaceAll(s, "\\t", "\t")
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\r", "\r")
	return s
}

var DigestPat = regexp.MustCompile(`[A-Za-z][A-Za-z0-9]*(?:[-_+.][A-Za-z][A-Za-z0-9]*)*[:][[:xdigit:]]{32,}`)

// and checking for panics
func ParseTsv(row string) ParseResult {
	// trim the trailing newline from the row
	fields := strings.Split(row, "\t")
	if len(fields) != 8 {
		panic(fmt.Sprintf("expected 8 fields, got %d:\n\"%s\"", len(fields), row))
	}
	return ParseResult{
		Input:         unescape(fields[0]),
		Name:          unescape(fields[1]),
		Domain:        unescape(fields[2]),
		Path:          unescape(fields[3]),
		Tag:           unescape(fields[4]),
		DigestAlgo:    unescape(fields[5]),
		DigestEncoded: unescape(fields[6]),
		Err:           unescape(strings.TrimRight(fields[7], "\n")),
	}
}

func (expected *ParseResult) Diff(actual *ParseResult) (string, bool) {
	same := true
	diff := strings.Builder{}
	diff.WriteString("--- expected\n+++ actual\n")
	if expected.Name != actual.Name {
		same = false
		MustWrite(&diff, fmt.Sprintf("- name: \"%s\"\n", expected.Name))
		MustWrite(&diff, fmt.Sprintf("+ name: \"%s\"\n", actual.Name))
	} else {
		MustWrite(&diff, fmt.Sprintf("  name: \"%s\"\n", expected.Name))
	}
	if expected.Domain != actual.Domain {
		same = false
		MustWrite(&diff, fmt.Sprintf("- domain: \"%s\"\n", expected.Domain))
		MustWrite(&diff, fmt.Sprintf("+ domain: \"%s\"\n", actual.Domain))
	} else {
		MustWrite(&diff, fmt.Sprintf("  domain: \"%s\"\n", expected.Domain))
	}
	if expected.Path != actual.Path {
		same = false
		MustWrite(&diff, fmt.Sprintf("- path: \"%s\"\n", expected.Path))
		MustWrite(&diff, fmt.Sprintf("+ path: \"%s\"\n", actual.Path))
	} else {
		MustWrite(&diff, fmt.Sprintf("  path: \"%s\"\n", expected.Path))
	}
	if expected.Tag != actual.Tag {
		same = false
		MustWrite(&diff, fmt.Sprintf("- tag: \"%s\"\n", expected.Tag))
		MustWrite(&diff, fmt.Sprintf("+ tag: \"%s\"\n", actual.Tag))
	} else {
		MustWrite(&diff, fmt.Sprintf("  tag: \"%s\"\n", expected.Tag))
	}
	if expected.DigestAlgo != actual.DigestAlgo {
		same = false
		MustWrite(&diff, fmt.Sprintf("- digestAlgo: \"%s\"\n", expected.DigestAlgo))
		MustWrite(&diff, fmt.Sprintf("+ digestAlgo: \"%s\"\n", actual.DigestAlgo))
	} else {
		MustWrite(&diff, fmt.Sprintf("  digestAlgo: \"%s\"\n", expected.DigestAlgo))
	}
	if expected.DigestEncoded != actual.DigestEncoded {
		same = false
		MustWrite(&diff, fmt.Sprintf("- digestEncoded: \"%s\"\n", expected.DigestEncoded))
		MustWrite(&diff, fmt.Sprintf("+ digestEncoded: \"%s\"\n", actual.DigestEncoded))
	} else {
		MustWrite(&diff, fmt.Sprintf("  digestEncoded: \"%s\"\n", expected.DigestEncoded))
	}
	if expected.Err != actual.Err {
		same = false
		MustWrite(&diff, fmt.Sprintf("- err: \"%s\"\n", expected.Err))
		MustWrite(&diff, fmt.Sprintf("+ err: \"%s\"\n", actual.Err))
	} else {
		MustWrite(&diff, fmt.Sprintf("  err: \"%s\"\n", expected.Err))
	}
	return diff.String(), same
}
func (expected *ParseResult) Pretty() string {
	result := strings.Builder{}
	MustWrite(&result, fmt.Sprintf("  input: \"%s\"\n", expected.Input))
	MustWrite(&result, fmt.Sprintf("  name: \"%s\"\n", expected.Name))
	MustWrite(&result, fmt.Sprintf("  domain: \"%s\"\n", expected.Domain))
	MustWrite(&result, fmt.Sprintf("  path: \"%s\"\n", expected.Path))
	MustWrite(&result, fmt.Sprintf("  tag: \"%s\"\n", expected.Tag))
	MustWrite(&result, fmt.Sprintf("  digestAlgo: \"%s\"\n", expected.DigestAlgo))
	MustWrite(&result, fmt.Sprintf("  digestEncoded: \"%s\"\n", expected.DigestEncoded))
	MustWrite(&result, fmt.Sprintf("  err: \"%s\"\n", expected.Err))
	return result.String()
}

func main() {
	validInputs, err := os.ReadFile("./tests/fixtures/references/valid/inputs.txt")
	PanicIf(err)
	validOutputs, err := os.Create("./tests/fixtures/references/outputs.tsv")
	PanicIf(err)
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
	MustWrite(&accumulator, header)
	parseFileLines(string(validInputs), &accumulator)

	// handle invalid inputs
	invalidInputs, err := os.ReadFile("./tests/fixtures/references/invalid/inputs.txt")
	PanicIf(err)
	parseFileLines(string(invalidInputs), &accumulator)

	// flush the accumulator to the output file
	MustWrite(validOutputs, accumulator.String())
}
