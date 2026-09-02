# Métas du dépôt : compilation croisée, exécution QEMU, packaging.
.PHONY: build-aarch64 build-riscv64 run-qemu help

help:
	@echo "Cibles :"
	@echo "  build-aarch64  Cross-compile Rust -> aarch64-unknown-linux-musl (RPi3B+)"
	@echo "  build-riscv64  Cross-compile Rust -> riscv64gc-unknown-linux-musl"
	@echo "  run-qemu       Execute le binaire ARM sous qemu-aarch64 (sans carte)"
	@echo "  flash-rpi3     Ecrire l'image Yocto sur la carte SD RPi3B+"

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

flash-rpi3:
	./scripts/flash-rpi3.sh build/tmp/deploy/images/raspberrypi3-64/__IMAGE__.wic /dev/sdX