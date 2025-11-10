# voidmerge makefile

.PHONY: all test bump

all: test

test:
	npm ci
	cargo fmt -- --check
	cargo clippy --locked -- -D warnings
	RUSTFLAGS="-D warnings" cargo test --locked --all-features
	(cd rs/voidmerge/ && cargo rdme --force)
	npm test

bump:
	@if [ "$(ver)x" = "x" ]; then \
		echo "USAGE: make bump ver=0.0.2"; \
		exit 1; \
	fi
	sed -i 's/^\(version = "\)[^"]*"/\1$(ver)"/g' ./Cargo.toml
	sed -i 's/^\(\s*"version": "\)[^"]*"/\1$(ver)"/g' ./ts/voidmerge-client/package.json
	sed -i 's/^\(\s*"version": "\)[^"]*"/\1$(ver)"/g' ./ts/voidmerge-code/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-client": "\)[^"]*"/\1^$(ver)"/g' ./ts/doc/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-code": "\)[^"]*"/\1^$(ver)"/g' ./ts/doc/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-code": "\)[^"]*"/\1^$(ver)"/g' ./ts/test-integration/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-client": "\)[^"]*"/\1^$(ver)"/g' ./ts/demo/todo-leader/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-code": "\)[^"]*"/\1^$(ver)"/g' ./ts/demo/todo-leader/package.json
	sed -i 's/^\(\s*"@voidmerge\/voidmerge-code": "\)[^"]*"/\1^$(ver)"/g' ./ts/demo/counter/package.json
	npm install
	cargo update
	$(MAKE) test
