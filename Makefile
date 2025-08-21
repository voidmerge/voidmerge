# voidmerge makefile

.PHONY: all test

all: test

test:
	cargo clippy -- -D warnings
	RUSTFLAGS="-D warnings" cargo test --all-features --all-targets
	npm ci
	npm test
