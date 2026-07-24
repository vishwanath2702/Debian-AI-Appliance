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
