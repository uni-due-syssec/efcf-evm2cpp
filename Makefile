PREFIX ?= /usr/local/

target/release/evm2cpp: $(wildcard src/*.rs)
	cargo build --release

build:
	cargo build

release-build:
	cargo build --release

test:
	cargo test --verbose

clean:
	cargo clean

install: target/release/evm2cpp
	install -s -m 755 -t $(PREFIX)/bin/ ./target/release/evm2cpp

uninstall:
	-$(RM) $(PREFIX)/bin/evm2cpp

upgrade:
	git pull
	cargo build --release
	sudo make install

.PHONY: build release-build workaround test clean install uninstall
