# Getting started

Three steps: your game files, your app, then the two together on your
device.

## 1. Your game files

GLUE 5 plays GTA V's **WebAssembly build**, the one made for web browsers.
Its folder looks like this:

```
game/
├── b/
│   └── 8b0b5899ed/
│       ├── game.wasm      ← the game's code: only needed to build the app
│       └── shaders/       ← its translated shaders
└── data/                  ← the game's data, prepared for that build
```

The build `8b0b5899ed` is the supported one: building checks `game.wasm`
against it. Files from a regular PC or console installation cannot be used
in place of these.

## 2. Your app

The app contains the game's code, compiled for your device, so it is
built from your own `game.wasm` and never downloaded from someone else.

- **Without a computer set up for it:** [build it on GitHub](github-build.md).
  It takes a GitHub account and a few clicks, and gives you the app for
  iPhone/iPad, Android, Mac, Windows or Linux.
- **On your computer:** see [Building from source](building.md).

## 3. Install it and add the files

The app needs the `shaders` and `data` folders on the device, or a
computer on the network that has them ([streaming](streaming.md)). The
first time you press Play, the app prepares the game's graphics pipelines:
a few minutes on a phone, about a second on later starts.

### iPhone and iPad

1. Install `Glue5-ios.ipa` with [Sideloadly](https://sideloadly.io) or
   [AltStore](https://altstore.io) and your Apple ID. With a free Apple
   ID, install it again every 7 days.
2. On the device, open **Settings › General › VPN & Device Management**
   and trust your developer profile, the first time.
3. Copy the `game` folder (with `data` and `b/…/shaders`) into the app's
   documents: on a Mac, **Finder › your device › Files › Glue 5**; on
   Windows, the Apple Devices app; or the **Files** app on the device.
   About 20 GB.

The app finds the files by itself when it comes back to the foreground.
The start page says what is missing, if anything.

### Android

1. Open the Android APK from the draft release on the phone and allow the
   install. Its name is `Glue5-android-<branch>-<commit>.apk`: the branch
   and commit identify the build (see [Build identity](build-identity.md)).
2. Give the app the game's files. **Use Find files to choose the game folder wherever it is (recommended).** On the start page, **Find files** (🗁) opens Android's folder picker: choose the folder that has `data` and `b/…/shaders`, or the folder holding it. The first time, allow **All files access** in system settings; Glue 5 reads the game in place and copies nothing.

   **Advanced option:** from a computer with USB debugging enabled, run `adb push game /sdcard/Android/data/local.glue5/files/` (with the `game` folder in the computer's current directory). This stores the files in Glue 5's app folder and needs no all-files permission. On Android 11 and later, do not try to copy into another app's `Android/data` folder with the phone's Files app or a SAF picker: Android blocks that access.

Keep `Glue5-android.keystore` from your GitHub build: the next version of
the app is signed with it, so it can update this one.

### Mac

1. Open `Glue5-macos.dmg` and drag GLUE 5 into Applications. The first
   time, macOS blocks it: open **System Settings › Privacy & Security** and
   press **Open Anyway**.
2. Put the `game` folder next to the app, or choose it with **Game folder**
   on the start page.

### Windows

1. Put `Glue5-windows.exe` in a folder of its own and run it. Windows warns
   about it the first time: **More info › Run anyway**. If it says
   `VCRUNTIME140.dll` is missing, install Microsoft's
   [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe).
2. Put the `game` folder next to it, or choose it with **Game folder** on
   the start page.

Your graphics driver needs Vulkan, which current NVIDIA, AMD and Intel
drivers have.

### Linux

1. Make `Glue5-linux-….AppImage` executable and run it.
2. Put the `game` folder next to it, or choose it on the start page.

It needs Vulkan from your graphics driver, and X11 or Wayland.

### Apple TV

The Apple TV keeps no game folder: it streams the files from a computer
on the same network, which shares them ([streaming](streaming.md)). It is
built and installed from a Mac with Xcode (`mise run tvos`, see
[Building from source](building.md)).

## Updating

Build the app again from the newer GLUE 5 ([on GitHub](github-build.md#get-a-new-version):
**Sync fork**, then build), and install it over the old one. Your saves,
settings and friends stay: they are kept apart from the app.

## Where your data is

Saves, settings, friends, characters and caches are in the app's user
folder:

| Device | Folder |
|---|---|
| Mac | `~/Library/Application Support/glue5` |
| Windows | `%APPDATA%\glue5` |
| Linux | `~/.local/share/glue5` |
| iPhone, iPad | the app's own storage (not visible in Files) |
| Android | the app's own storage |

Mods are in `mods` there on computers, and next to the `game` folder on
phones and tablets (where Files and the Finder reach them).
