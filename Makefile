# Métas du dépôt : compilation, tests, exécution QEMU, versant kernel.
.PHONY: build build-aarch64 build-riscv64 test run-qemu kernel help

help:
	@echo "Cibles :"
	@echo "  build          Compile la release locale (x86_64, natif)"
	@echo "  test           Tests unitaires (cargo test, release)"
	@echo "  build-aarch64  Cross-compile Rust -> aarch64-unknown-linux-musl (RPi3B+)"
	@echo "  build-riscv64  Cross-compile Rust -> riscv64gc-unknown-linux-musl"
	@echo "  run-qemu       Execute le binaire ARM sous qemu-aarch64 (sans carte)"
	@echo "  kernel         Build du module noyau + overlay (voir kernel/)"
	@echo "  flash-rpi3     Ecrire l'image Yocto sur la carte SD RPi3B+"

build:
	docker run --rm -v "$$PWD":/work -w /work rust:latest cargo build --release

test:
	docker run --rm -v "$$PWD":/work -w /work rust:latest cargo test --release

build-aarch64:
	docker run --rm -v "$$PWD":/work -w /work \
	  ghcr.io/rust-cross/rust-musl-cross:aarch64-musl \
	  cargo build --release --target aarch64-unknown-linux-musl

build-riscv64:
	docker run --rm -v "$$PWD":/work -w /work \
	  ghcr.io/rust-cross/rust-musl-cross:riscv64gc-musl \
	  cargo build --release --target riscv64gc-unknown-linux-musl

run-qemu:
	./scripts/run-qemu.sh 5555

kernel:
	$(MAKE) -C kernel

flash-rpi3:
	./scripts/flash-rpi3.sh build/tmp/deploy/images/raspberrypi3-64/__IMAGE__.wic /dev/sdX