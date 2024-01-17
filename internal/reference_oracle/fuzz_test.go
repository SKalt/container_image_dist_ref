package main

import (
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"testing"
	"unicode/utf8"
)

func unescape(s string) string {
	s = strings.ReplaceAll(s, "\\t", "\t")
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\r", "\r")
	return s
}

var digestPat = regexp.MustCompile(`[A-Za-z][A-Za-z0-9]*(?:[-_+.][A-Za-z][A-Za-z0-9]*)*[:][[:xdigit:]]{32,}`)

// and checking for panics
func parseTsv(row string) parseResult {
	// trim the trailing newline from the row
	fields := strings.Split(row, "\t")
	if len(fields) != 8 {
		panic(fmt.Sprintf("expected 8 fields, got %d:\n\"%s\"", len(fields), row))
	}
	row = strings.TrimRight(row, "\n")
	return parseResult{
		input:         unescape(fields[0]),
		name:          unescape(fields[1]),
		domain:        unescape(fields[2]),
		path:          unescape(fields[3]),
		tag:           unescape(fields[4]),
		digestAlgo:    unescape(fields[5]),
		digestEncoded: unescape(fields[6]),
		err:           unescape(strings.TrimRight(fields[7], "\n")),
	}
}

func (expected *parseResult) diff(actual *parseResult) (string, bool) {
	same := true
	diff := strings.Builder{}
	diff.WriteString("--- expected\n+++ actual\n")
	if expected.name != actual.name {
		same = false
		mustWrite(&diff, fmt.Sprintf("- name: \"%s\"\n", expected.name))
		mustWrite(&diff, fmt.Sprintf("+ name: \"%s\"\n", actual.name))
	} else {
		mustWrite(&diff, fmt.Sprintf("  name: \"%s\"\n", expected.name))
	}
	if expected.domain != actual.domain {
		same = false
		mustWrite(&diff, fmt.Sprintf("- domain: \"%s\"\n", expected.domain))
		mustWrite(&diff, fmt.Sprintf("+ domain: \"%s\"\n", actual.domain))
	} else {
		mustWrite(&diff, fmt.Sprintf("  domain: \"%s\"\n", expected.domain))
	}
	if expected.path != actual.path {
		same = false
		mustWrite(&diff, fmt.Sprintf("- path: \"%s\"\n", expected.path))
		mustWrite(&diff, fmt.Sprintf("+ path: \"%s\"\n", actual.path))
	} else {
		mustWrite(&diff, fmt.Sprintf("  path: \"%s\"\n", expected.path))
	}
	if expected.tag != actual.tag {
		same = false
		mustWrite(&diff, fmt.Sprintf("- tag: \"%s\"\n", expected.tag))
		mustWrite(&diff, fmt.Sprintf("+ tag: \"%s\"\n", actual.tag))
	} else {
		mustWrite(&diff, fmt.Sprintf("  tag: \"%s\"\n", expected.tag))
	}
	if expected.digestAlgo != actual.digestAlgo {
		same = false
		mustWrite(&diff, fmt.Sprintf("- digestAlgo: \"%s\"\n", expected.digestAlgo))
		mustWrite(&diff, fmt.Sprintf("+ digestAlgo: \"%s\"\n", actual.digestAlgo))
	} else {
		mustWrite(&diff, fmt.Sprintf("  digestAlgo: \"%s\"\n", expected.digestAlgo))
	}
	if expected.digestEncoded != actual.digestEncoded {
		same = false
		mustWrite(&diff, fmt.Sprintf("- digestEncoded: \"%s\"\n", expected.digestEncoded))
		mustWrite(&diff, fmt.Sprintf("+ digestEncoded: \"%s\"\n", actual.digestEncoded))
	} else {
		mustWrite(&diff, fmt.Sprintf("  digestEncoded: \"%s\"\n", expected.digestEncoded))
	}
	if expected.err != actual.err {
		same = false
		mustWrite(&diff, fmt.Sprintf("- err: \"%s\"\n", expected.err))
		mustWrite(&diff, fmt.Sprintf("+ err: \"%s\"\n", actual.err))
	} else {
		mustWrite(&diff, fmt.Sprintf("  err: \"%s\"\n", expected.err))
	}
	return diff.String(), same
}
func (expected *parseResult) pretty() string {
	result := strings.Builder{}
	mustWrite(&result, fmt.Sprintf("  input: \"%s\"\n", expected.input))
	mustWrite(&result, fmt.Sprintf("  name: \"%s\"\n", expected.name))
	mustWrite(&result, fmt.Sprintf("  domain: \"%s\"\n", expected.domain))
	mustWrite(&result, fmt.Sprintf("  path: \"%s\"\n", expected.path))
	mustWrite(&result, fmt.Sprintf("  tag: \"%s\"\n", expected.tag))
	mustWrite(&result, fmt.Sprintf("  digestAlgo: \"%s\"\n", expected.digestAlgo))
	mustWrite(&result, fmt.Sprintf("  digestEncoded: \"%s\"\n", expected.digestEncoded))
	mustWrite(&result, fmt.Sprintf("  err: \"%s\"\n", expected.err))
	return result.String()
}

const binPath = "../../target/debug/examples/parse_stdin"

func ipv6ExpectedFailure(errName string) bool {
	return strings.HasPrefix(errName, "Ipv6") && (strings.HasPrefix(errName, "Ipv6TooLong") ||
		strings.HasPrefix(errName, "Ipv6BadColon") ||
		strings.HasPrefix(errName, "Ipv6TooManyHexDigits") ||
		strings.HasPrefix(errName, "Ipv6TooManyGroups") ||
		strings.HasPrefix(errName, "Ipv6TooFewGroups"))
}

func harness(t *testing.T, input string) {
	// skip the test if the input is invalid utf8
	if !utf8.ValidString(input) {
		return
	}
	oracle := parse(input)
	t.Logf("input: \"%s\"", input)

	cmd := exec.Cmd{Path: binPath, Stdin: strings.NewReader(input + "\n")}
	resultBytes, err := cmd.Output()
	if err != nil { // rust lib errored
		if e, ok := err.(*exec.ExitError); ok {
			switch e.ExitCode() {
			case 0:
				t.Fatal("unreachable")
			case 1:
				// normal rust lib error
				result := parseTsv(string(resultBytes))
				if oracle.err == "" { // distribution/reference parsed successfully
					// the rust lib differs from the go lib by constraining IPv6 addresses
					if ipv6ExpectedFailure(result.err) {
						return
					}
					t.Errorf("unexpected error:\n%s\n\n%s", result.err, oracle.pretty())
					return
				} else {
					// ok: distribution/reference errored just like the rust lib did
					return
				}
			default:
				// the rust lib panicked
				t.Error(string(e.Stderr))
				break
			}
		} else if _, ok := err.(*fs.PathError); ok {
			cwd, _ := os.Getwd()
			t.Fatalf("unable to find %s\nwrong cwd: %s", binPath, cwd)

		} else {
			// unexpected error
			t.Fatal(err)
		}
	} else {
		// the rust lib parsed successfully
		result := parseTsv(string(resultBytes))
		diff, same := oracle.diff(&result)
		if oracle.err != "" { // distribution/reference errored
			switch result.digestAlgo {
			case "sha256":
			case "sha512":
				// unexpected error, distribution/reference supports support these
				// check the pattern:
				if digestPat.Match([]byte(result.digestAlgo + ":" + result.digestEncoded)) {
					t.Log("matched?")
				}
				t.Errorf("unexpected error in registered algorithm:\n%s", diff)
				return
			default:
				// expected error: distribution/reference can't handle non-registered algorithms
				return
			}
		} else { // distribution/reference parsed successfully
			if !same {
				t.Errorf("diff:\n%s", diff)
			}
			return
		}
	}
}

func FuzzAnyParsing(f *testing.F) {
	data, err := os.ReadFile("./inputs.txt")
	panicIf(err)
	lines := strings.Split(string(data), "\n")
	for _, line := range lines {
		if line != "" {
			f.Add(line)
		}
	}
	f.Fuzz(harness)
}

// TODO: use seed data from the fuzzing corpus

// func FuzzSeeded(f *testing.F) {
// 	data, err := os.ReadFile("./inputs.txt")
// 	panicIf(err)
// 	lines := strings.Split(string(data), "\n")
// 	for _, line := range lines {
// 		if line != "" {
// 			f.Add(line)
// 		}
// 	}
// 	f.Fuzz(harness)
// }
