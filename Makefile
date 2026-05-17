BINARY       ?= pixelcreep
CARGO        ?= cargo
REPO_ROOT    := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
# Detect fast linker flags
ifeq ($(shell command -v clang >/dev/null 2>&1 && command -v mold >/dev/null 2>&1 && echo 1),1)
  LINKER_FLAGS := -C linker=clang -C link-arg=-fuse-ld=mold
else ifeq ($(shell command -v clang >/dev/null 2>&1 && command -v lld >/dev/null 2>&1 && echo 1),1)
  LINKER_FLAGS := -C linker=clang -C link-arg=-fuse-ld=lld
else
  LINKER_FLAGS :=
endif

DEV_RUSTFLAGS     := $(strip $(RUSTFLAGS) $(LINKER_FLAGS))
RELEASE_RUSTFLAGS := $(strip $(RUSTFLAGS) -C target-cpu=native)

.PHONY: dev build release run clean

dev:
	cd "$(REPO_ROOT)" && CARGO_INCREMENTAL=1 RUSTFLAGS="$(DEV_RUSTFLAGS)" $(CARGO) build --profile dev-fast
	cd "$(REPO_ROOT)" && ./target/dev-fast/$(BINARY)

build:
	cd "$(REPO_ROOT)" && RUSTFLAGS="$(RELEASE_RUSTFLAGS)" $(CARGO) build --release
	cd "$(REPO_ROOT)" && ./target/release/$(BINARY)

release: build

run: dev

clean:
	cd "$(REPO_ROOT)" && $(CARGO) clean
