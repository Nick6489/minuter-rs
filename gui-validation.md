# Native GUI validation

Minuter 0.2.0 uses rungui 0.4.0. In addition to providing audible time cues, it serves as a small real-world test of the toolkit's native controls and accessibility in a background application.

The earlier comparison measured 4,627,968 bytes for rungui versus 11,200,000 bytes for the WXDragon v0.1.0 release: a 58.7% reduction. Release metadata can cause small size differences in the final v0.2.0 executable. Both distributions consist of one executable, with the original sound assets embedded and only Windows system DLL imports.

## Windows checks

Five audio/preferences tests pass. The native smoke test checks accessible checkbox/button/combo-box roles, descriptive names, checked state, preference saving, output-device switching, spoken seconds through NVDA, Ctrl+Alt+Shift+Backspace, tray reopening, Escape, Enter, and clean exit.

```powershell
cargo test --release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./verify-native.ps1
```

These are automated native MSAA checks and speech-output checks. Full manual use of the interface with NVDA or Narrator remains a separate check.

## Integration notes

rungui currently has no tray or default-button API. `src/tray.rs` supplies Windows notification-area integration and the native default-button ID. Close is a direct native child so the Windows dialog manager can activate it with Enter. Escape uses `Window::on_cancel`.

The settings window is a `RunguiWindow` top-level window, rather than the `#32770` dialog used by v0.1.0. Its controls expose native names, roles, and checked state.

rungui retains widgets and callbacks after `App::run` returns. Minuter explicitly destroys its window before process teardown so callback-held audio and speech workers can stop and join cleanly.

The interface, audio engine, and speech binding provide cross-platform foundations. The tray/default-button adapter and global shortcut are currently Windows-specific. macOS packaging, tray integration, and VoiceOver testing remain future work. The GUI dependency has no C++ build; Prism's speech implementation still requires native build tools.

References: [rungui repository](https://github.com/graydon/rungui), [platform status and known gaps](https://github.com/graydon/rungui/blob/main/doc/STATUS.md).
