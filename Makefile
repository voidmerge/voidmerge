# voidmerge makefile

.PHONY: all test bump

all: test

test:
	cargo clippy -- -D warnings
	RUSTFLAGS="-D warnings" cargo test --all-features --all-targets
	npm ci
	npm test

bump:
	@if [ "$(ver)x" = "x" ]; then \
		echo "USAGE: make bump ver=0.0.2"; \
		exit 1; \
	fi
	sed -i 's/^\(version = "\)[^"]*"/\1$(ver)"/g' ./Cargo.toml
	sed -i 's/^\(\s*"version": "\)[^"]*"/\1$(ver)"/g' ./ts/voidmerge-client/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-client": "\)[^"]*"/\1^$(ver)"/g' ./ts/example1/package.json
	npm install
	$(MAKE) test
