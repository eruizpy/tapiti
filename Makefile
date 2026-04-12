TARGET   := aarch64-linux-android
NDK_HOME ?= $(if $(ANDROID_NDK_HOME),$(ANDROID_NDK_HOME),$(shell ls -d ~/Library/Android/sdk/ndk/* ~/Android/Sdk/ndk/* /opt/android-sdk/ndk/* /opt/android-sdk/ndk-bundle 2>/dev/null | tail -1))
TOOLCHAIN ?= $(firstword $(wildcard $(NDK_HOME)/toolchains/llvm/prebuilt/*))
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
	@if [ -z "$(NDK_HOME)" ] || [ ! -x "$(CC)" ]; then \
		echo "Android NDK not found. Set ANDROID_NDK_HOME or NDK_HOME."; \
		exit 1; \
	fi
	CC=$(CC) AR=$(AR) CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$(CC) cargo build --release --target $(TARGET)

copy: build
	mkdir -p $(APK_ASSETS)
	cp $(BINARY) $(APK_ASSETS)/tapiti

android-debug: copy
	cd android && ./gradlew assembleDebug

clean:
	cargo clean
	rm -f $(APK_ASSETS)/tapiti
