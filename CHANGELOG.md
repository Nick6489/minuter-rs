# Changelog

## 0.2.0

- Switch the released interface from WXDragon to rungui, keeping the cue options, embedded sounds, screen-reader speech, device picker, tray controls, and global shortcut.
- Reduce the Windows executable size substantially while retaining a one-file distribution.
- Use the standard Minuter window/tray names and existing `%APPDATA%\Minuter\settings.json` preferences.
- Add Windows tray/default-button integration and explicit widget cleanup for clean shutdown.
- Make Minuter a real-world native-GUI validation app for rungui.

## 0.1.0

First standalone Windows release, based on Andre Louis's Minuter NVDA add-on.

- Original tick and minute-chime sounds embedded in one executable.
- Second ticks, low beeps, end-of-minute warnings, minute chimes, and spoken seconds.
- Accessible WXDragon settings dialog with immediate changes and saved preferences.
- Sound-device selection, automatic system-default tracking, and a test-sound button.
- Speech through an active screen reader or system voice.
- System tray controls and Ctrl+Alt+Shift+Backspace to show or hide settings.
- Continuous audio mixing without a backlog of cues after sleep.

The executable is built for Windows x64. macOS packaging and testing remain future work.
