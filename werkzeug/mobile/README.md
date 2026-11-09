# Mobile toolchain

Run from the repository root after `pnpm install --frozen-lockfile`:

```sh
bash werkzeug/mobile/build-android.sh
```

The script builds the current mobile UI, syncs it to Capacitor, and builds a debug-signed APK:
`apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`.
Install with `adb install -r apps/mobile/android/app/build/outputs/apk/debug/app-debug.apk`.
Launch with `adb shell am start -n dev.orchester.mobile/.MainActivity`.

To run the Android bridge tests on a running emulator or connected test device:

```sh
source werkzeug/mobile/android-env.sh
adb devices -l
cd apps/mobile/android
ANDROID_SERIAL=emulator-5554 ./gradlew --no-daemon :app:connectedDebugAndroidTest
```

Replace the serial with the device listed by `adb`. Use the `:app:` task prefix:
the generated Cordova dependency's own test project is not the application's test
suite. Reports are written to `app/build/reports/androidTests/connected/debug`.
`NativeBridgeTest` launches the packaged WebView, checks the actual Credentials
plugin through JavaScript, recreates the Activity, verifies encrypted storage and
device-key binding, and sends an authenticated HTTP request through Capacitor's
native networking stack to a test server without CORS headers. It does not call a
model provider or prove that a physical phone can reach a computer over Wi-Fi.

For emulator troubleshooting, use
`"$ANDROID_HOME/emulator/emulator" -accel-check`. A successful APK build does not
require an emulator. Linux acceleration requires usable KVM permissions;
Android Studio's Windows SDK and the WSL/Linux SDK are separate installations.

Launcher icons, the native launch screen and the short animated launch overlay
share `apps/desktop/src-tauri/icons/icon.png`. The generated PNGs are checked in;
normal app builds do not need an image library. After changing the desktop icon,
regenerate them with Python and Pillow:

```sh
python3 werkzeug/mobile/generate-brand-assets.py
```

The web transition uses the selected theme, releases its overlay after about
1.2 seconds, and respects reduced motion. Resuming an existing WebView does not
replay it. The overlay starts independently of device reconnects.

Requirements: Node 22+, pnpm, JDK 21, Android SDK platform 36 and build-tools 35.0.0/36.0.0.
Gradle is downloaded by the checked-in wrapper. `android-env.sh` uses the isolated
`~/.cache/orchester-android/{jdk,sdk}` toolchain when JAVA_HOME / ANDROID_HOME are unset;
ORCHESTER_ANDROID_ROOT can override this cache location. It does not install software.
With Android command-line tools already present, missing SDK packages can be installed with:

```sh
source werkzeug/mobile/android-env.sh
sdkmanager 'platform-tools' 'platforms;android-36' 'build-tools;35.0.0' 'build-tools;36.0.0'
```

Android credentials use AES-GCM with an Android Keystore key; iOS uses device-only
Keychain entries. Both implement the JS Credentials get/set/remove contract.
The apps allow HTTP traffic to user-selected LAN servers. Use HTTPS for untrusted networks.
Android has INTERNET permission and permits mixed content; iOS declares a local network
usage description and App Transport Security exceptions. Device firewall, LAN reachability,
and iOS local-network permission still apply.

For iOS, use macOS with Xcode and a signing team:

```sh
pnpm --filter @orchester/mobile build
pnpm --filter @orchester/mobile exec cap sync ios
pnpm --filter @orchester/mobile exec cap open ios
```

Linux can inspect and synchronize the iOS project, but cannot compile, sign, or validate
its runtime behavior. No connected Android device is assumed by the build script.
