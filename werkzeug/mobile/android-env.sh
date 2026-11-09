#!/usr/bin/env bash
# Source to use an isolated local Android toolchain; never changes system Java.
ORCHESTER_ANDROID_ROOT="${ORCHESTER_ANDROID_ROOT:-${XDG_CACHE_HOME:-$HOME/.cache}/orchester-android}"
export JAVA_HOME="${JAVA_HOME:-$ORCHESTER_ANDROID_ROOT/jdk}"
export ANDROID_HOME="${ANDROID_HOME:-$ORCHESTER_ANDROID_ROOT/sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  failed=0
  for tool in java javac sdkmanager adb; do
    if command -v "$tool" >/dev/null 2>&1; then
      printf '%s: %s\n' "$tool" "$(command -v "$tool")"
    else
      printf 'Missing: %s\n' "$tool" >&2
      failed=1
    fi
  done
  for artifact in platforms/android-36/android.jar build-tools/36.0.0/aapt2; do
    if [[ ! -f "$ANDROID_HOME/$artifact" ]]; then
      printf 'Missing SDK artifact: %s\n' "$artifact" >&2
      failed=1
    fi
  done
  exit "$failed"
fi
