#!/usr/bin/env bash
set -euo pipefail
mobile_tools_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$mobile_tools_dir/../.." && pwd)"
source "$mobile_tools_dir/android-env.sh"
bash "$mobile_tools_dir/android-env.sh"
cd "$repo_dir"
pnpm --filter @orchester/mobile build
pnpm --filter @orchester/mobile exec cap sync android
cd "$repo_dir/apps/mobile/android"
./gradlew --no-daemon :app:assembleDebug
printf '\nAPK: %s\n' "$PWD/app/build/outputs/apk/debug/app-debug.apk"
