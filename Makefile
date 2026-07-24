PROJECT := $(CURDIR)

all: build

extract:
	./build/extract.sh

inject:
	./build/inject.sh

build:
	./build/rebuild.sh

clean:
	./build/cleanup.sh

test:
	qemu-system-x86_64 \
	-m 4096 \
	-enable-kvm \
	-cdrom output/debian-ai.iso

.PHONY: all build clean fmt clippy test shellcheck check

fmt:
	cd engine && cargo fmt --check

clippy:
	cd engine && cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cd engine && cargo test --workspace --all-features

shellcheck:
	./build/shellcheck.sh

check: fmt clippy test shellcheck
