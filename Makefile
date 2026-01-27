.PHONY: build run test clean install example

build:
	cargo build --release

run:
	cargo run -- generate examples/

test:
	cargo test

clean:
	cargo clean

install:
	cargo install --path .

example:
	cargo run -- generate examples/sample.ts
	@echo "\n📄 Generated frontmatter:"
	@head -10 examples/sample.ts
