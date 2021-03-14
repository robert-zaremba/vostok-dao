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
	@cargo doc
# remove all files
#	for i in $$(ls docs); do if [ -f $$i ]; then rm "docs/$$i"; fi; done;
	for i in $$(ls docs); do test -f docs/$$i && rm "docs/$$i" ; done;
	@rm -rf docs/api
	@cp -r target/doc/vostok_dao/ docs/
	@for i in ayu.css brush.svg favicon* light.css main.js normalize.css noscript.css rustdoc.css rust-logo.png search-index.js source* storage.js theme.js wheel.svg ; do cp target/doc/$$i docs/; done;

build:
	@env 'RUSTFLAGS=-C link-arg=-s' cargo build --all --lib --target wasm32-unknown-unknown --release
	@cp target/wasm32-unknown-unknown/release/*.wasm ./res/
