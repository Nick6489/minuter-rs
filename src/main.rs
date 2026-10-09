#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod audio;
mod hotkey;
mod settings;
mod speech;

use audio::{Audio, Command};
use settings::Settings;
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};
use wxdragon::{
    art_provider::{ArtClient, ArtId, ArtProvider},
    prelude::*,
    single_instance_checker::SingleInstanceChecker,
    timer::Timer,
};

struct Runtime {
    timer: Rc<Timer<Frame>>,
    tray: Rc<TaskBarIcon>,
    _audio: Rc<Audio>,
    _speech: Rc<speech::Speech>,
    _hotkey: Option<Rc<hotkey::Hotkey>>,
    _instance: SingleInstanceChecker,
    _menu: Menu,
}
#[cfg(windows)]
thread_local! { static PREVIOUS_WINDOW: std::cell::Cell<isize> = const { std::cell::Cell::new(0) }; }
fn show(dialog: Dialog, first: CheckBox) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let previous = GetForegroundWindow();
        if previous.0 != dialog.get_handle() {
            PREVIOUS_WINDOW.with(|p| p.set(previous.0 as isize));
        }
    }
    dialog.show(true);
    dialog.raise();
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
            windows::Win32::Foundation::HWND(dialog.get_handle()),
        );
    }
    first.set_focus();
}
fn hide(dialog: Dialog) {
    #[cfg(windows)]
    let foreground = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 == dialog.get_handle()
    };
    dialog.show(false);
    #[cfg(windows)]
    if foreground {
        PREVIOUS_WINDOW.with(|p| {
            if p.get() != 0 {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                        windows::Win32::Foundation::HWND(p.get() as *mut std::ffi::c_void),
                    );
                }
            }
        });
    }
}
fn save(settings: &Rc<RefCell<Settings>>, audio: &Audio, status: StaticText) {
    let current = settings.borrow();
    audio.flags.store(current.mask(), Ordering::Relaxed);
    if let Err(e) = current.save() {
        status.set_label(&e);
    }
}
fn populate(choice: Choice, settings: &Settings) -> Result<Vec<String>, String> {
    let mut names = audio::devices()?;
    if let Some(selected) = &settings.device {
        if !names.contains(selected) {
            names.push(selected.clone());
        }
    }
    choice.clear();
    choice.append("System default");
    for name in &names {
        choice.append(name);
    }
    let selected = settings
        .device
        .as_ref()
        .and_then(|s| names.iter().position(|n| n == s))
        .map(|i| i as u32 + 1)
        .unwrap_or(0);
    choice.set_selection(selected);
    Ok(names)
}
fn main() {
    let runtime: Rc<RefCell<Option<Runtime>>> = Rc::new(RefCell::new(None));
    let keep = runtime.clone();

    let result = wxdragon::main(move |app| {
        app.set_app_name("Minuter");
        app.set_app_display_name("Minuter");
        app.set_exit_on_frame_delete(false);
        let frame = Frame::builder()
            .with_title("Minuter background")
            .with_style(FrameStyle::NoTaskbar)
            .build();
        let Some(instance) = SingleInstanceChecker::new("minuter-standalone", None) else {
            app.set_exit_on_frame_delete(true);
            frame.destroy();
            app.exit_main_loop();
            return;
        };
        if instance.is_another_running() {
            let message = MessageDialog::builder(
                &frame,
                "Minuter is already running. Use its system tray icon to show settings.",
                "Minuter",
            )
            .build();
            message.show_modal();
            app.set_exit_on_frame_delete(true);
            frame.destroy();
            app.exit_main_loop();
            return;
        }
        let (settings, load_error) = match Settings::load() {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        let state = Rc::new(RefCell::new(settings));
        let audio = match Audio::start(state.borrow().device.clone(), state.borrow().mask()) {
            Ok(a) => Rc::new(a),
            Err(e) => {
                MessageDialog::builder(&frame, &e, "Minuter")
                    .build()
                    .show_modal();
                app.set_exit_on_frame_delete(true);
                frame.destroy();
                app.exit_main_loop();
                return;
            }
        };
        let speech = Rc::new(speech::Speech::start(audio.flags.clone()));
        let hotkey = hotkey::Hotkey::register();
        let hotkey_error = hotkey.as_ref().err().cloned();
        let hotkey = hotkey.ok().map(Rc::new);
        let dialog = Dialog::builder(&frame, "Minuter")
            .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
            .build();
        app.set_top_window(&dialog);
        let layout = BoxSizer::builder(Orientation::Vertical).build();
        let seconds =
            StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, &dialog, "Second cues")
                .build();
        let second_parent = seconds.get_static_box().unwrap();
        let tick = CheckBox::builder(&second_parent)
            .with_label("&Tick every second")
            .build();
        let low = CheckBox::builder(&second_parent)
            .with_label("Beep &low (250 Hz) every second")
            .build();
        let warning = CheckBox::builder(&second_parent)
            .with_label("&Warning beeps at 55–59 (1 kHz)")
            .build();
        let speak = CheckBox::builder(&second_parent)
            .with_label("&Speak seconds")
            .build();
        let cues = [tick, low, warning, speak];
        for cue in cues {
            seconds.add(&cue, 0, SizerFlag::All, 6);
        }
        layout.add_sizer(&seconds, 0, SizerFlag::Expand | SizerFlag::All, 8);
        let minute_group =
            StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, &dialog, "Minute cue")
                .build();
        let minute_parent = minute_group.get_static_box().unwrap();
        let minute = CheckBox::builder(&minute_parent)
            .with_label("&Ding at the top of each minute")
            .build();
        minute_group.add(&minute, 0, SizerFlag::All, 6);
        layout.add_sizer(
            &minute_group,
            0,
            SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
            8,
        );
        let all_cues = [tick, low, warning, speak, minute];
        {
            let s = state.borrow();
            for (cue, value) in all_cues
                .into_iter()
                .zip([s.tick, s.low, s.warning, s.speak, s.minute])
            {
                cue.set_value(value);
            }
        }
        let actions = BoxSizer::builder(Orientation::Horizontal).build();
        let enable = Button::builder(&dialog).with_label("&Enable all").build();
        let disable = Button::builder(&dialog).with_label("D&isable all").build();
        actions.add(&enable, 0, SizerFlag::All, 6);
        actions.add(&disable, 0, SizerFlag::All, 6);
        layout.add_sizer(
            &actions,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
            8,
        );
        let output =
            StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, &dialog, "Sound output")
                .build();
        let output_parent = output.get_static_box().unwrap();
        let label = StaticText::builder(&output_parent)
            .with_label("Sound &device:")
            .build();
        let choice = Choice::builder(&output_parent)
            .with_size(Size::new(460, -1))
            .build();
        choice.set_name("Sound device");
        choice.set_accessibility_label("Sound device");
        let row = BoxSizer::builder(Orientation::Horizontal).build();
        let refresh = Button::builder(&output_parent)
            .with_label("&Refresh devices")
            .build();
        let test = Button::builder(&output_parent)
            .with_label("Test s&ound")
            .build();
        let help = StaticText::builder(&output_parent)
            .with_label("Speech follows your screen reader or system voice output.")
            .build();
        output.add(&label, 0, SizerFlag::All, 6);
        output.add(
            &choice,
            0,
            SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right,
            6,
        );
        row.add(&refresh, 0, SizerFlag::All, 6);
        row.add(&test, 0, SizerFlag::All, 6);
        output.add_sizer(&row, 0, SizerFlag::Expand, 0);
        output.add(
            &help,
            0,
            SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
            6,
        );
        layout.add_sizer(
            &output,
            0,
            SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
            8,
        );
        let audio_status = StaticText::builder(&dialog)
            .with_label("Opening sound output…")
            .build();
        let speech_status = StaticText::builder(&dialog)
            .with_label("Speech follows your screen reader or system voice.")
            .build();
        let status = StaticText::builder(&dialog)
            .with_label("Settings apply immediately.")
            .build();
        for text in [audio_status, speech_status, status] {
            layout.add(
                &text,
                0,
                SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
                14,
            );
        }
        let shortcut = StaticText::builder(&dialog)
            .with_label("Ctrl+Alt+Shift+Backspace shows or hides settings.\nClose keeps cues running. The tray icon also reopens settings.")
            .build();
        layout.add(&shortcut, 0, SizerFlag::All, 14);
        let buttons = BoxSizer::builder(Orientation::Horizontal).build();
        let quit = Button::builder(&dialog).with_label("&Quit Minuter").build();
        let close = Button::builder(&dialog)
            .with_id(ID_CLOSE)
            .with_label("&Close")
            .build();
        close.set_default();
        dialog.set_affirmative_id(ID_CLOSE);
        dialog.set_escape_id(ID_CLOSE);
        buttons.add(&quit, 0, SizerFlag::All, 6);
        buttons.add_stretch_spacer(1);
        buttons.add(&close, 0, SizerFlag::All, 6);
        layout.add_sizer(&buttons, 0, SizerFlag::Expand | SizerFlag::All, 8);
        dialog.set_sizer(layout, true);
        dialog.fit();
        dialog.centre();
        let names = Rc::new(RefCell::new(Vec::new()));
        match populate(choice, &state.borrow()) {
            Ok(n) => *names.borrow_mut() = n,
            Err(e) => status.set_label(&e),
        }
        if let Some(e) = load_error.or(hotkey_error) {
            status.set_label(&e);
        }
        for cue in all_cues {
            let (state, audio) = (state.clone(), audio.clone());
            cue.on_toggled(move |_| {
                {
                    let mut s = state.borrow_mut();
                    s.tick = tick.get_value();
                    s.low = low.get_value();
                    s.warning = warning.get_value();
                    s.speak = speak.get_value();
                    s.minute = minute.get_value();
                }
                save(&state, &audio, status);
            });
        }
        for (button, enabled) in [(enable, true), (disable, false)] {
            let (state, audio) = (state.clone(), audio.clone());
            button.on_click(move |_| {
                for cue in all_cues {
                    cue.set_value(enabled);
                }
                state.borrow_mut().set_all(enabled);
                save(&state, &audio, status);
            });
        }
        {
            let (state, audio, names) = (state.clone(), audio.clone(), names.clone());
            choice.on_selection_changed(move |_| {
                let selected = choice.get_selection().unwrap_or(0);
                let device = if selected == 0 {
                    None
                } else {
                    names.borrow().get(selected as usize - 1).cloned()
                };
                state.borrow_mut().device = device.clone();
                let _ = audio.tx.send(Command::Device(device));
                save(&state, &audio, status);
            });
        }
        {
            let (state, names) = (state.clone(), names.clone());
            refresh.on_click(move |_| match populate(choice, &state.borrow()) {
                Ok(n) => {
                    *names.borrow_mut() = n;
                    status.set_label("Device list refreshed.");
                }
                Err(e) => status.set_label(&e),
            });
        }
        {
            let audio = audio.clone();
            test.on_click(move |_| {
                audio.preview.store(1, Ordering::Relaxed);
            });
        }
        close.on_click(move |_| hide(dialog));
        dialog.on_close(move |event| {
            hide(dialog);
            event.skip(false);
        });
        // The character hook ensures Escape/Enter work even when a child control owns focus.
        dialog.bind_internal(EventType::CHAR_HOOK, move |event| {
            if event.get_key_code() == Some(WXK_ESCAPE) {
                hide(dialog);
            } else {
                event.skip(true);
            }
        });
        let tray = Rc::new(
            TaskBarIcon::builder()
                .with_tooltip("Minuter — second and minute cues")
                .build(),
        );
        if let Some(icon) = ArtProvider::get_bitmap(
            ArtId::Information,
            ArtClient::FrameIcon,
            Some(Size::new(32, 32)),
        ) {
            tray.set_icon(&icon, "Minuter — second and minute cues");
            dialog.set_icon(&icon);
        }
        let mut menu = Menu::builder()
            .append_item(10001, "Show settings", "Show Minuter settings")
            .append_separator()
            .append_item(10002, "Disable all", "Turn off every cue")
            .append_item(10003, "Quit Minuter", "Stop cues and exit")
            .build();
        tray.set_popup_menu(&mut menu);
        tray.on_left_double_click(move |_| show(dialog, tick));
        let timer = Rc::new(Timer::new(&frame));
        let quitting = Rc::new(std::cell::Cell::new(false));
        {
            let (audio, speech) = (audio.clone(), speech.clone());
            let hotkey = hotkey.clone();
            let quitting = quitting.clone();
            let cleanup = keep.clone();
            timer.on_tick(move |_| {
                if quitting.get() {
                    if let Some(r) = cleanup.borrow_mut().take() {
                        r.timer.stop();
                        r.tray.remove_icon();
                        r.tray.destroy();
                        drop(r);
                    }
                    app.exit_main_loop();
                    return;
                }
                if hotkey.as_ref().is_some_and(|h| h.pending_toggle()) {
                    if dialog.is_shown() {
                        hide(dialog);
                    } else {
                        show(dialog, tick);
                    }
                }
                while let Ok(message) = audio.status.try_recv() {
                    audio_status.set_label(&message);
                }
                while let Ok(message) = speech.status.try_recv() {
                    speech_status.set_label(&message);
                }
            });
        }
        timer.start(100, false);
        {
            let quitting = quitting.clone();
            quit.on_click(move |_| quitting.set(true));
        }
        {
            let (state, audio) = (state.clone(), audio.clone());
            let quitting = quitting.clone();
            tray.on_menu(move |event| match event.get_id() {
                10001 => show(dialog, tick),
                10002 => {
                    for cue in all_cues {
                        cue.set_value(false);
                    }
                    state.borrow_mut().set_all(false);
                    save(&state, &audio, status);
                }
                10003 => quitting.set(true),
                _ => (),
            });
        }
        show(dialog, tick);
        *keep.borrow_mut() = Some(Runtime {
            timer,
            tray,
            _audio: audio,
            _speech: speech,
            _hotkey: hotkey,
            _menu: menu,
            _instance: instance,
        });
    });
    if let Err(e) = result {
        eprintln!("Minuter could not start: {e:?}");
    }
}
