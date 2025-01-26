.PHONY: fuzz
fuzz: cli
	(cd internal/reference_oracle && go test -fuzz=.)

fuzz-canonical: cli
	(cd internal/fuzz_canonical && go test -fuzz=.)

.PHONY: cli
cli:
	cargo build --examples

.PHONY: bench
bench:
	cargo bench

.PHONY: test
test:
	cargo test --features=check_no_panic

./grammars/digest.diff:      \
	./grammars/reference.ebnf  \
	./grammars/oci_digest.ebnf \
	./scripts/diff_digest.sh   \

	./scripts/diff_digest.sh > ./grammars/digest.diff

./grammars/digest_algorithm.diff:          \
	./grammars/digest.diff                   \
	./scripts/diff_digest_algorithm.sh       \

	./scripts/diff_digest_algorithm.sh > ./grammars/digest_algorithm.diff

./grammars/digest_encoded.diff:    \
	./grammars/digest.diff           \
	./scripts/diff_digest_encoded.sh \

	./scripts/diff_digest_encoded.sh > ./grammars/digest_encoded.diff

./grammars/host_subset.ebnf: \
	./grammars/reference.ebnf  \
	./scripts/host_subset.sh   \

	./scripts/host_subset.sh > ./grammars/host_subset.ebnf

./grammars/host_or_path.ebnf:      \
	./grammars/reference.ebnf        \
	./scripts/subset_host_or_path.sh \

	./scripts/subset_host_or_path.sh > ./grammars/host_or_path.ebnf

.PHONY: grammars
grammars:                          \
	./grammars/digest.diff           \
	./grammars/digest_algorithm.diff \
	./grammars/digest_encoded.diff   \
	./grammars/host_subset.ebnf      \
	./grammars/host_or_path.ebnf     \

.PHONY: link-check
link-check:
	cargo doc
	lychee --exclude=./target/package .

rust-docs:
	cargo doc --open
