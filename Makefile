.PHONY: fuzz
fuzz: cli
	(cd internal/reference_oracle && go test -fuzz=.)

.PHONY: cli
cli:
	cargo build --examples

.PHONY: bench
bench:
	cargo bench
