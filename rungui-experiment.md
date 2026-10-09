# rungui experiment

The experimental Windows build uses rungui 0.4.0 instead of WXDragon. The original WXDragon interface remains the default build on this local `experiment/rungui` branch. The public v0.1.0 release is unchanged.

## Run and build

Run `experimental-dist/Minuter.exe`. Its folder contains only that executable, with both original sounds embedded.

Build it with `./build-rungui.ps1`, or use:

```powershell
cargo build --release --locked --no-default-features --features rungui-ui --bin minuter-rungui
```

The experiment saves preferences in `%APPDATA%\Minuter-rungui\settings.json`, separately from the published app. Both versions use Ctrl+Alt+Shift+Backspace; quit one before comparing the other's global shortcut. A shortcut collision is reported in the settings window.

## Measured comparison

| Check | WXDragon release | rungui experiment |
| --- | --- | --- |
| Executable size | 11,200,000 bytes | 4,627,968 bytes |
| Distribution | One executable | One executable |
| Embedded sounds | Original tick and minute chime | Same original assets |
| Windows settings class | `#32770` dialog | `RunguiWindow` top-level window |
| Native checkbox/button/combo roles | Pass | Pass |
| Accessible names and checked state | Pass | Pass |
| Preferences and output-device switching | Pass | Pass |
| Spoken seconds through NVDA | Pass | Pass |
| Global shortcut, tray reopening, Escape, Enter | Pass | Pass |
| Clean exit | Pass | Pass |

The experiment is 58.7% smaller. Imports were inspected: it needs only Windows system libraries, with no accompanying toolkit DLLs or sound files.

## Integration findings

The interface reuses the existing audio engine, speech worker, global shortcut, and preference format. The toolkit provides the groups, buttons, checkboxes, combo box, labels, layout, and UI timer.

rungui currently has no tray API or default-button API. `src/tray_rungui.rs` supplies a small Windows adapter using native notification-area APIs and a default-button ID. The Close button is a direct child of the top-level window so the Windows dialog manager can locate it when Enter is pressed. Escape uses rungui's `Window::on_cancel`.

rungui retains widgets and their callbacks after `App::run` returns. The experiment explicitly destroys its window before process teardown so callback-held audio and speech workers are released while they can still stop and join. This fixed a shutdown crash found by the smoke test.

## Verification and remaining comparison

Five audio/settings tests pass for the experimental binary. The shared Windows smoke test passes for both interfaces:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./verify-native.ps1 -Rungui
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./verify-native.ps1
```

The accessibility checks inspect native MSAA names, roles, and checked state; spoken seconds were sent successfully through NVDA. These checks do not substitute for manually using the entire interface with NVDA or Narrator. The top-level window is a regular window rather than the original dialog, which should be included in that comparison.

This experiment is Windows-tested. Its tray/default-button adapter is Windows-specific, and macOS packaging, tray integration, and VoiceOver testing remain future work.

The result is a promising fit for this small app: native controls, a substantially smaller executable, and a GUI dependency without a C++ build. Prism's speech implementation still requires its native build tools.

Toolkit references: [rungui repository](https://github.com/graydon/rungui), [platform status and known gaps](https://github.com/graydon/rungui/blob/main/doc/STATUS.md).
