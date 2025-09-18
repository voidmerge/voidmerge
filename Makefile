# voidmerge makefile

.PHONY: all test bump

all: test

test:
	cargo fmt -- --check
	cargo clippy --locked -- -D warnings
	RUSTFLAGS="-D warnings" cargo test --locked --all-features --all-targets
	(cd rs/voidmerge/ && cargo rdme --force)
	npm ci
	npm test
	@if [ "${CI}x" != "x" ]; then git diff --exit-code; fi

bump:
	@if [ "$(ver)x" = "x" ]; then \
		echo "USAGE: make bump ver=0.0.2"; \
		exit 1; \
	fi
	sed -i 's/^\(version = "\)[^"]*"/\1$(ver)"/g' ./Cargo.toml
	sed -i 's/^\(\s*"version": "\)[^"]*"/\1$(ver)"/g' ./ts/voidmerge-client/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-client": "\)[^"]*"/\1^$(ver)"/g' ./ts/example1/package.json
	npm install
	cargo update
	$(MAKE) test
