# Build the app on GitHub

GitHub builds the app for you, for free. You only need a GitHub account
and your `game.wasm` file.

> ⚠️ **Two rules:** never press **Publish release**. Always press **Save
> draft**. A draft is private: only you can see it. And never put
> `game.wasm` anywhere else: not in your copy's files (**Add file**, a
> commit), not behind a download link (not even one in a secret), not in
> the Actions' artifacts. Those are public, and the build does not read it
> from there.

## The first time

**1. Copy this project**

Press **Fork** at the top of this page, then **Create fork**.

**2. Turn on the builds**

In your copy, open the **Actions** tab and press the green button
(**I understand my workflows, go ahead and enable them**).

**3. Add your game.wasm**

In your copy, open **Releases** (on the right side of the main page),
then **Draft a new release**.

- Title: anything, for example `game`.
- Drag `game.wasm` into the box at the bottom. The right one is
  63,201,802 bytes. Zipped is fine too, named `game.wasm.zip`.
- Wait until it has finished uploading.
- Press **Save draft**. ⚠️ Not **Publish release**.

## Build the app

1. Open the **Actions** tab.
2. On the left, press **Build**.
3. On the right, press **Run workflow**, choose your device (`ios`,
   `android`, `windows`, `macos`, `linux-x86_64` or `linux-aarch64`),
   press the green **Run workflow**.
4. Wait: you can close the page and come back later. A green ✅
   means it is done.
5. Open **Releases**, then your draft: the app is there, next to
   `game.wasm`. Download it.

If something is missing or wrong, the build stops and the red ❌ tells
you what to do: open it and read the message.

If it is cancelled because the job was "not acquired by a runner",
GitHub had no free machine: run it again later.

## Install the app

Every build's files carry the branch and the first seven characters of
its commit, e.g. `Glue5-android-arena-107599ac-glue5-8767ebd.apk`. The
`Glue5-BUILD-<branch>-<commit>.txt` file in the draft records branch,
commit, subject, platform, time and the Actions run. See
[Build identity](build-identity.md).

- **iPhone / iPad** (`Glue5-ios-….ipa`): install it with
  [Sideloadly](https://sideloadly.io) or [AltStore](https://altstore.io)
  and your Apple ID. With a free Apple ID you have to install it again
  every 7 days.
- **Android** (`Glue5-android-….apk`): open the file on the phone and
  install it. ⚠️ Do not delete `Glue5-android.keystore` from the draft:
  without it, the next version cannot update the app.
- **Windows** (`Glue5-windows-….exe`): put it in a folder of its own and
  run it. The first time, Windows warns about it: press **More info**,
  then **Run anyway**. If it says `VCRUNTIME140.dll` is missing, install
  Microsoft's
  [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe).
- **Mac** (`Glue5-macos-….dmg`): open it and drag the app into
  Applications. The first time, macOS blocks it: open **System Settings ›
  Privacy & Security** and press **Open Anyway**.
- **Linux** (`Glue5-linux-…-<branch>-<commit>.AppImage`): make it executable and run it.

Then put the game's files on the device: see
[Getting started](getting-started.md#3-install-it-and-add-the-files).

## Get a new version

1. On the main page of your copy, press **Sync fork**, then **Update
   branch**.
2. Build the app again (see above). Your `game.wasm` is still there.

## If you published by mistake

Nothing to do: within a few seconds GitHub puts it back to draft by
itself, and you get an email about it. In those few seconds anyone could
have downloaded the files, so try not to do it.

