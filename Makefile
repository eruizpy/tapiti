TARGET   := aarch64-linux-android
NDK_HOME ?= $(shell ls -d ~/Library/Android/sdk/ndk/* 2>/dev/null | tail -1)
TOOLCHAIN := $(NDK_HOME)/toolchains/llvm/prebuilt/darwin-x86_64
CC       := $(TOOLCHAIN)/bin/aarch64-linux-android21-clang
AR       := $(TOOLCHAIN)/bin/llvm-ar

BINARY   := target/$(TARGET)/release/tapiti
APK_ASSETS := android/app/src/main/assets

.PHONY: all check build copy android-debug clean

all: check

check:
	cargo fmt
	cargo clippy -- -D warnings
	cargo test
	cargo check

build:
	CC=$(CC) AR=$(AR) cargo build --release --target $(TARGET)

copy: build
	mkdir -p $(APK_ASSETS)
	cp $(BINARY) $(APK_ASSETS)/tapiti

android-debug: copy
	cd android && ./gradlew assembleDebug

clean:
	cargo clean
	rm -f $(APK_ASSETS)/tapiti
