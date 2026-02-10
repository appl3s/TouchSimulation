.PHONY: all push
CLANG_RT_DIR := $(shell find $(ANDROID_NDK_HOME) -name "libclang_rt.builtins-aarch64-android.a" -print -quit | xargs dirname)

all:
	RUSTFLAGS="-L${CLANG_RT_DIR} -C link-arg=-lclang_rt.builtins-aarch64-android" \
	cargo ndk -t aarch64-linux-android build --release --verbose

push:
	adb push target/aarch64-linux-android/release/touch_simulation /data/local/tmp/touch_simulation