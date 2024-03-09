package fuzz_canonical

import (
	"io/fs"
	"os"
	"os/exec"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/skalt/container_image_dist_ref/internal/reference_oracle"
)

const canonicalRefParserPath = "../../target/debug/examples/parse_canonical"

func canonicalHarness(t *testing.T, input string) {
	// skip the test if the input is invalid utf8
	if !utf8.ValidString(input) {
		return
	}
	input = strings.TrimRight(input, "\r\n")
	oracle := reference_oracle.ParseCanonical(input)
	t.Logf("input: \"%s\"", input)

	cmd := exec.Cmd{Path: canonicalRefParserPath, Stdin: strings.NewReader(input + "\n")}
	resultBytes, err := cmd.Output()
	if err != nil { // rust lib errored
		if e, ok := err.(*exec.ExitError); ok {
			switch e.ExitCode() {
			case 0:
				t.Fatal("unreachable")
			case 1:
				// normal rust lib error
				result := reference_oracle.ParseTsv(string(resultBytes))
				if oracle.Err == "" { // distribution/reference parsed successfully
					// the rust lib differs from the go lib by constraining IPv6 addresses
					if reference_oracle.Ipv6ExpectedFailure(result.Err) {
						return
					}
					t.Errorf("unexpected error:\n%s\n\n%s", result.Err, oracle.Pretty())
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
			t.Fatalf("unable to find %s\nwrong cwd: %s", canonicalRefParserPath, cwd)

		} else {
			// unexpected error
			t.Fatal(err)
		}
	} else {
		// the rust lib parsed successfully
		result := reference_oracle.ParseTsv(string(resultBytes))
		diff, same := oracle.Diff(&result)
		if oracle.Err != "" { // distribution/reference errored
			switch result.DigestAlgo {
			case "sha256":
			case "sha512":
				// unexpected error, distribution/reference supports support these
				// check the pattern:
				if reference_oracle.DigestPat.Match([]byte(result.DigestAlgo + ":" + result.DigestEncoded)) {
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

func FuzzCanonicalParsing(f *testing.F) {
	data, err := os.ReadFile("./inputs.txt")
	reference_oracle.PanicIf(err)
	lines := strings.Split(string(data), "\n")
	for _, line := range lines {
		if line != "" {
			f.Add(line)
		}
	}
	f.Fuzz(canonicalHarness)
}
