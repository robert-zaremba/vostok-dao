test:
# to test specific test run: cargo test <test name>
	@cargo test

test-debug:
# "--" allows to pass extra arguments
# "--nocapture" disables stdout capturing (testes print all println)
	@RUST_BACKTRACE=1 cargo test  -- --nocapture

test-unit:
	@cargo test --lib
# run specific tests: cargo test --lib <testname>  -- --nocapture


build-doc:
	cargo doc

build:
	@env 'RUSTFLAGS=-C link-arg=-s' cargo build --all --lib --target wasm32-unknown-unknown --release
	@cp target/wasm32-unknown-unknown/release/*.wasm ./res/
