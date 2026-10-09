#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod audio;
mod hotkey;
mod settings;
mod speech;
mod tray_rungui;

use audio::{Audio, Command};
use rungui::*;
use settings::Settings;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::Ordering,
};

#[cfg(windows)]
thread_local! {static PREVIOUS:Cell<isize>=const {Cell::new(0)};}
fn show(window: Window, first: CheckBox) {
    #[cfg(windows)]
    unsafe {
        let old = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if old.0 as usize != window.native_handle().map(|h| h.ptr()).unwrap_or(0) {
            PREVIOUS.with(|p| p.set(old.0 as isize));
        }
    }
    window.show();
    #[cfg(windows)]
    unsafe {
        if let Some(handle) = window.native_handle() {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                windows::Win32::Foundation::HWND(handle.ptr() as *mut _),
            );
        }
    }
    first.focus();
}
fn hide(window: Window) {
    #[cfg(windows)]
    let foreground = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 as usize
            == window.native_handle().map(|h| h.ptr()).unwrap_or(0)
    };
    window.hide();
    #[cfg(windows)]
    if foreground {
        PREVIOUS.with(|p| unsafe {
            if p.get() != 0 {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                    windows::Win32::Foundation::HWND(p.get() as *mut _),
                );
            }
        });
    }
}
fn save(state: &Rc<RefCell<Settings>>, audio: &Audio, status: Label) {
    let settings = state.borrow();
    audio.flags.store(settings.mask(), Ordering::Relaxed);
    if let Err(e) = settings.save() {
        status.set_text(&e);
    }
}
fn populate(choice: ComboBox, state: &Settings) -> std::result::Result<Vec<String>, String> {
    let mut names = audio::devices()?;
    if let Some(selected) = &state.device {
        if !names.contains(selected) {
            names.push(selected.clone());
        }
    }
    let mut items = vec!["System default".to_string()];
    items.extend(names.clone());
    choice.set_items(&items);
    let index = state
        .device
        .as_ref()
        .and_then(|s| names.iter().position(|n| n == s))
        .map(|i| i + 1)
        .unwrap_or(0);
    choice.set_selected(Some(index));
    Ok(names)
}
fn fail(text: &str) {
    message_box(None, MessageKind::Error, Buttons::Ok, "Minuter", text);
}
fn main() {
    let app = match App::new("Minuter-rungui") {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e:?}");
            return;
        }
    };
    let _instance = match tray_rungui::Instance::acquire() {
        Ok(i) => i,
        Err(e) => {
            fail(&e);
            return;
        }
    };
    App::set_quit_on_last_close(false);
    let (saved, load_error) = match Settings::load() {
        Ok(s) => (s, None),
        Err(e) => (Settings::default(), Some(e)),
    };
    let state = Rc::new(RefCell::new(saved));
    let audio = match Audio::start(state.borrow().device.clone(), state.borrow().mask()) {
        Ok(a) => Rc::new(a),
        Err(e) => {
            fail(&e);
            return;
        }
    };
    let speech = Rc::new(speech::Speech::start(audio.flags.clone()));
    let hotkey = hotkey::Hotkey::register();
    let hotkey_error = hotkey.as_ref().err().cloned();
    let hotkey = hotkey.ok().map(Rc::new);
    let window = Window::new("Minuter — rungui experiment");
    window.set_resizable(true);
    window.set_padding(12);
    window.set_spacing(8);
    let content = VBox::new(window);
    content.set_spacing(10);
    let seconds = GroupBox::new(content, "Second cues");
    seconds.set_padding(12);
    seconds.set_spacing(8);
    let tick = CheckBox::new(seconds, "&Tick every second");
    let low = CheckBox::new(seconds, "Beep &low (250 Hz) every second");
    let warning = CheckBox::new(seconds, "&Warning beeps at 55–59 (1 kHz)");
    let speak = CheckBox::new(seconds, "&Speak seconds");
    let minute_group = GroupBox::new(content, "Minute cue");
    minute_group.set_padding(12);
    let minute = CheckBox::new(minute_group, "&Ding at the top of each minute");
    let cues = [tick, low, warning, speak, minute];
    {
        let s = state.borrow();
        for (cue, value) in cues
            .into_iter()
            .zip([s.tick, s.low, s.warning, s.speak, s.minute])
        {
            cue.set_checked(value);
        }
    }
    let actions = HBox::new(content);
    actions.set_spacing(10);
    let enable = Button::new(actions, "&Enable all");
    let disable = Button::new(actions, "D&isable all");
    let output = GroupBox::new(content, "Sound output");
    output.set_padding(12);
    output.set_spacing(8);
    Label::new(output, "Sound device:");
    let choice = ComboBox::new(output);
    choice.set_min_size(460, 0);
    choice.set_a11y_name("Sound device");
    let row = HBox::new(output);
    row.set_spacing(10);
    let refresh = Button::new(row, "&Refresh devices");
    let test = Button::new(row, "Test s&ound");
    Label::new(
        output,
        "Speech follows your screen reader or system voice output.",
    );
    let audio_status = Label::new(content, "Opening sound output…");
    let speech_status = Label::new(
        content,
        "Speech follows your screen reader or system voice.",
    );
    let status = Label::new(content, "Settings apply immediately.");
    Label::new(content, "Ctrl+Alt+Shift+Backspace shows or hides settings.");
    Label::new(
        content,
        "Close keeps cues running. The tray icon reopens settings.",
    );
    let quit = Button::new(content, "&Quit Minuter");
    quit.set_align(Align::Start);
    // A direct native child lets Windows locate this button by its dialog default ID.
    let close = Button::new(window, "&Close");
    close.set_align(Align::End);
    let names = Rc::new(RefCell::new(Vec::new()));
    match populate(choice, &state.borrow()) {
        Ok(n) => *names.borrow_mut() = n,
        Err(e) => status.set_text(&e),
    }
    if let Some(e) = load_error.or(hotkey_error) {
        status.set_text(&e);
    }
    for cue in cues {
        let (state, audio) = (state.clone(), audio.clone());
        cue.on_toggle(move |_| {
            {
                let mut s = state.borrow_mut();
                s.tick = tick.checked();
                s.low = low.checked();
                s.warning = warning.checked();
                s.speak = speak.checked();
                s.minute = minute.checked();
            }
            save(&state, &audio, status);
        });
    }
    for (button, value) in [(enable, true), (disable, false)] {
        let (state, audio) = (state.clone(), audio.clone());
        button.on_click(move || {
            for cue in cues {
                cue.set_checked(value);
            }
            state.borrow_mut().set_all(value);
            save(&state, &audio, status);
        });
    }
    {
        let (state, audio, names) = (state.clone(), audio.clone(), names.clone());
        choice.on_select(move |selection| {
            let device = selection
                .filter(|i| *i > 0)
                .and_then(|i| names.borrow().get(i - 1).cloned());
            state.borrow_mut().device = device.clone();
            let _ = audio.tx.send(Command::Device(device));
            save(&state, &audio, status);
        });
    }
    {
        let (state, names) = (state.clone(), names.clone());
        refresh.on_click(move || match populate(choice, &state.borrow()) {
            Ok(n) => {
                *names.borrow_mut() = n;
                status.set_text("Device list refreshed.");
            }
            Err(e) => status.set_text(&e),
        });
    }
    {
        let audio = audio.clone();
        test.on_click(move || {
            audio.preview.store(1, Ordering::Relaxed);
        });
    }
    close.on_click(move || hide(window));
    window.on_cancel(move || hide(window));
    window.on_close(move || {
        hide(window);
        false
    });
    let quitting = Rc::new(Cell::new(false));
    {
        let quitting = quitting.clone();
        quit.on_click(move || quitting.set(true));
    }
    App::update();
    let tray = match tray_rungui::Tray::new(
        window.native_handle().map(|h| h.ptr()).unwrap_or(0),
        close.native_handle().map(|h| h.ptr()).unwrap_or(0),
    ) {
        Ok(t) => Some(Rc::new(t)),
        Err(e) => {
            status.set_text(&e);
            None
        }
    };
    let timer_slot = Rc::new(Cell::new(None::<Timer>));
    let timer = Timer::every(100, {
        let (audio, speech, hotkey, state, quitting, timer_slot) = (
            audio.clone(),
            speech.clone(),
            hotkey.clone(),
            state.clone(),
            quitting.clone(),
            timer_slot.clone(),
        );
        let tray = tray.as_ref().map(Rc::downgrade);
        move || {
            let native_tray = tray.as_ref().and_then(|t| t.upgrade());
            let commands = native_tray.as_ref().map(|t| t.commands()).unwrap_or(0);
            if commands & 4 != 0 {
                quitting.set(true);
            }
            if quitting.get() {
                if let Some(t) = timer_slot.get() {
                    t.stop();
                }
                if let Some(t) = native_tray {
                    t.detach();
                }
                App::quit();
                return;
            }
            if commands & 1 != 0 {
                show(window, tick);
            }
            if commands & 2 != 0 {
                for cue in cues {
                    cue.set_checked(false);
                }
                state.borrow_mut().set_all(false);
                save(&state, &audio, status);
            }
            if hotkey.as_ref().is_some_and(|h| h.pending_toggle()) {
                if window.visible() {
                    hide(window);
                } else {
                    show(window, tick);
                }
            }
            while let Ok(text) = audio.status.try_recv() {
                audio_status.set_text(&text);
            }
            while let Ok(text) = speech.status.try_recv() {
                speech_status.set_text(&text);
            }
        }
    });
    timer_slot.set(Some(timer));
    show(window, tick);
    app.run();
    timer.stop();
    if let Some(tray) = tray {
        tray.detach();
    }
    // rungui retains widgets after App::run. Release their callbacks before Windows
    // process teardown, while Rust audio/speech/hotkey workers can still be joined.
    window.destroy();
}
