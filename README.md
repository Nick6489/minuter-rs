# Minuter

Standalone Rust version of Andre Louis's Minuter time-cue add-on. Uses WXDragon for the interface and Prismer for screen-reader or system-voice speech.

## Run

Download `Minuter.exe` from the [latest release](https://github.com/Nick6489/minuter-rs/releases/latest), then open it. No installation, NVDA, companion DLLs, or separate sound files are required.

- Tick every second: the original tick, trimmed to 120 milliseconds.
- Low beep: 250 Hz for 50 milliseconds each second.
- Warning beeps: 1 kHz for 50 milliseconds at seconds 55–59.
- Speak seconds: Prismer uses an available screen reader or system voice.
- Minute chime: the original sound at the top of each minute.
- Enable all and Disable all apply immediately.

Choose a sound device or System default, then use Test sound to check it. The device selection affects ticks, chimes, and beeps. Speech follows the screen reader's or system voice's output settings. System default follows device changes automatically; a disconnected selected device is retried automatically.

Ctrl+Alt+Shift+Backspace shows or hides settings globally on Windows. If another application has reserved the shortcut, Minuter reports it in settings and the tray remains available.

Close, Escape, or the title-bar close button hides settings while cues continue. Double-click the Minuter tray icon to reopen settings, or use its menu for Show settings, Disable all, and Quit Minuter. Enter activates Close. Quit stops the app.

Preferences are saved in `%APPDATA%\Minuter\settings.json`. All cues start disabled on the first run. The distributed app consists of one executable; its preferences file is created when settings change.

## Build

Requires Rust, Visual Studio C++ build tools, CMake, and LLVM/libclang on Windows. Run `./build.ps1` from PowerShell. The script builds a release executable and copies only `Minuter.exe` into `dist`.

WXDragon, Prism/Prismer, and the C/C++ runtime are statically linked. The original WAV bytes are included at compile time, decoded once in memory, and resampled when opening an output device. A continuous CPAL audio callback mixes the cues without file access, locks, or allocation. Clock changes and sleep do not produce a backlog of missed cues.

Run `cargo test --release` for audio and preference checks. Run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./verify-native.ps1` for the Windows dialog/tray smoke test. It restores preferences after testing.

The interface, sound engine, and speech binding are cross-platform foundations. This build is tested on Windows; macOS packaging and platform validation remain future work.

## Credits

Original Minuter add-on and sounds: [Andre Louis's Minuter](https://github.com/OnjLouis/Minuter). Sound assets were copied unchanged from the provided add-on version 1.0.2.

Interface: WXDragon and wxWidgets. Speech: Prismer and Prism. Audio: CPAL. The original add-on source has been left unchanged.

## Experimental rungui interface

This local branch also contains a [rungui experiment and measured comparison](rungui-experiment.md). Run ./build-rungui.ps1 to produce its separate one-file Windows build. The default build remains WXDragon.
