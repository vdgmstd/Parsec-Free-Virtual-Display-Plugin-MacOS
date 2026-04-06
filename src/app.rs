use dioxus::prelude::*;
use dioxus::desktop::use_window;
use std::cell::RefCell;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;
use objc2_foundation::MainThreadMarker;
use crate::commands::{take_commands, Command};
use crate::settings::{Settings, CustomResolution, RESOLUTIONS, FRAME_RATES};
use crate::display::{DisplayConfig, ResolutionMode, VirtualDisplay};
use crate::tray::NativeTray;

// Thread-local storage for tray icon (required since NSStatusItem isn't Send/Sync)
thread_local! {
    static TRAY: RefCell<Option<NativeTray>> = const { RefCell::new(None) };
}

fn init_tray() {
    TRAY.with(|tray| {
        if tray.borrow().is_none() {
            // Safe because Dioxus desktop runs on the main thread
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            *tray.borrow_mut() = Some(NativeTray::new(mtm));
        }
    });
}

fn update_tray_status(connected: bool, username: Option<&str>) {
    TRAY.with(|tray| {
        if let Some(ref t) = *tray.borrow() {
            t.set_status(connected, username);
            t.set_connected(connected);
        }
    });
}

const STYLE: &str = include_str!("../assets/style.css");

// Get version from DioxusConfig (embedded PVDisplayPlugin.toml)
fn get_version() -> String {
    crate::dioxus_config::DioxusConfig::load()
        .ok()
        .and_then(|cfg| cfg.application.version.clone())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// Build DisplayConfig with all available resolution modes
fn build_display_config(settings: &Settings) -> DisplayConfig {
    // Collect all available modes: preset + custom
    let mut modes: Vec<ResolutionMode> = RESOLUTIONS
        .iter()
        .map(|&(w, h, _)| ResolutionMode { width: w, height: h })
        .collect();

    // Add custom resolutions
    for custom in &settings.custom_resolutions {
        if !modes.iter().any(|m| m.width == custom.width && m.height == custom.height) {
            modes.push(ResolutionMode {
                width: custom.width,
                height: custom.height,
            });
        }
    }

    DisplayConfig {
        width: settings.width,
        height: settings.height,
        frame_rate: settings.frame_rate,
        name: settings.display_name.clone(),
        available_modes: modes,
    }
}

#[derive(Clone, PartialEq)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub username: Option<String>,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self {
            connected: false,
            username: None,
        }
    }
}

fn get_parsec_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".parsec")
        .join("log.txt")
}

fn parse_parsec_line(line: &str) -> Option<(bool, String)> {
    // Pattern: "Username#12345678 connected." or "Username#12345678 disconnected."
    if line.contains(" connected.") {
        if let Some(user_part) = line.split(" connected.").next() {
            if let Some(username) = user_part.split(']').last() {
                let username = username.trim();
                if !username.is_empty() {
                    return Some((true, username.to_string()));
                }
            }
        }
    }

    if line.contains(" disconnected.") {
        if let Some(user_part) = line.split(" disconnected.").next() {
            if let Some(username) = user_part.split(']').last() {
                let username = username.trim();
                if !username.is_empty() {
                    return Some((false, username.to_string()));
                }
            }
        }
    }

    None
}

#[component]
pub fn App() -> Element {
    let mut settings = use_signal(|| Settings::load().unwrap_or_default());
    let mut status = use_signal(ConnectionStatus::default);
    let mut toast_message = use_signal(|| None::<(String, bool)>); // (message, is_success)
    let mut display = use_signal(|| None::<VirtualDisplay>);
    let mut log_position = use_signal(|| 0u64);
    let mut is_saved = use_signal(|| true);

    // Custom resolution input state
    let mut custom_width = use_signal(|| String::from("1920"));
    let mut custom_height = use_signal(|| String::from("1080"));

    // App version from PVDisplayPlugin.toml
    let version = get_version();

    // Initialize tray icon
    use_effect(|| {
        init_tray();
    });

    // Tray command handler - processes commands from tray menu
    let _command_handler = use_coroutine(move |_rx: UnboundedReceiver<()>| async move {
        // Helper to recreate display with current settings
        let recreate_display = |display: &mut Signal<Option<VirtualDisplay>>, settings: &Signal<Settings>| {
            if display.peek().is_some() {
                if let Some(mut vd) = display.take() {
                    let _ = vd.destroy();
                }
                let s = settings.peek().clone();
                let config = build_display_config(&s);
                let mut vd = VirtualDisplay::new(config);
                if vd.create().is_ok() {
                    display.set(Some(vd));
                }
            }
        };

        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;

            for cmd in take_commands() {
                tracing::debug!("[App] Processing tray command: {:?}", cmd);

                match cmd {
                    Command::SetResolution(w, h) => {
                        settings.write().width = w;
                        settings.write().height = h;
                        is_saved.set(false);
                        recreate_display(&mut display.clone(), &settings);
                        TRAY.with(|tray| {
                            if let Some(ref t) = *tray.borrow() {
                                t.set_resolution(w, h);
                            }
                        });
                    }
                    Command::SetFps(fps) => {
                        settings.write().frame_rate = fps;
                        is_saved.set(false);
                        recreate_display(&mut display.clone(), &settings);
                        TRAY.with(|tray| {
                            if let Some(ref t) = *tray.borrow() {
                                t.set_fps(fps);
                            }
                        });
                    }
                }
            }
        }
    });

    // Parsec log monitoring coroutine
    let _parsec_monitor = use_coroutine(move |_rx: UnboundedReceiver<()>| async move {
        let log_path = get_parsec_log_path();

        // Initial seek to end of file
        if let Ok(file) = File::open(&log_path) {
            if let Ok(metadata) = file.metadata() {
                log_position.set(metadata.len());
            }
        }

        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Ok(mut file) = File::open(&log_path) {
                let current_pos = log_position();
                if file.seek(SeekFrom::Start(current_pos)).is_ok() {
                    let reader = BufReader::new(&mut file);
                    let mut new_pos = current_pos;

                    for line in reader.lines().map_while(Result::ok) {
                        new_pos += line.len() as u64 + 1; // +1 for newline

                        if let Some((connected, username)) = parse_parsec_line(&line) {
                            tracing::info!(
                                "Parsec event: {} {}",
                                if connected { "CONNECTED" } else { "DISCONNECTED" },
                                username
                            );
                            status.set(ConnectionStatus {
                                connected,
                                username: Some(username.clone()),
                            });

                            // Update tray icon status
                            update_tray_status(connected, Some(&username));

                            // Auto-create/destroy display if enabled
                            let current_settings = settings.peek().clone();
                            if current_settings.auto_create {
                                if connected && display.peek().is_none() {
                                    // Create display with all resolution modes
                                    let config = build_display_config(&current_settings);
                                    let mut vd = VirtualDisplay::new(config);
                                    let _ = vd.create();
                                    display.set(Some(vd));
                                } else if !connected && display.peek().is_some() {
                                    // Destroy display
                                    if let Some(mut vd) = display.take() {
                                        let _ = vd.destroy();
                                    }
                                }
                            }
                        }
                    }

                    log_position.set(new_pos);
                }
            }
        }
    });

    // Helper to show toast notification
    let mut show_toast = move |msg: String, success: bool| {
        toast_message.set(Some((msg, success)));
        spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            toast_message.set(None);
        });
    };

    // Recreate display with current settings (if active)
    let mut recreate_display_if_active = move || {
        if display.peek().is_some() {
            // Destroy current
            if let Some(mut vd) = display.take() {
                let _ = vd.destroy();
            }
            // Create new with updated settings and all resolution modes
            let current_settings = settings.peek().clone();
            let config = build_display_config(&current_settings);
            let mut vd = VirtualDisplay::new(config);
            if vd.create().is_ok() {
                display.set(Some(vd));
                show_toast(format!("Display: {}x{}", current_settings.width, current_settings.height), true);
            }
        }
    };

    let save_settings = move |_| {
        if let Err(e) = settings.read().save() {
            show_toast(format!("Error: {}", e), false);
        } else {
            is_saved.set(true);
        }
    };

    let manual_create = move |_| {
        let current_settings = settings.peek().clone();
        let config = build_display_config(&current_settings);
        let mut vd = VirtualDisplay::new(config);
        match vd.create() {
            Ok(()) => {
                display.set(Some(vd));
                show_toast("Display created!".to_string(), true);
            }
            Err(e) => {
                show_toast(format!("Error: {}", e), false);
            }
        }
    };

    let manual_destroy = move |_| {
        if let Some(mut vd) = display.take() {
            match vd.destroy() {
                Ok(()) => {
                    show_toast("Display destroyed!".to_string(), true);
                }
                Err(e) => {
                    show_toast(format!("Error: {}", e), false);
                }
            }
        }
    };

    let status_text = if status().connected {
        format!("Connected: {}", status().username.unwrap_or_default())
    } else {
        "Disconnected".to_string()
    };

    let status_class = if status().connected {
        "status-dot connected"
    } else {
        "status-dot"
    };

    let display_active = display.read().is_some();

    // Build resolution options (preset + custom)
    let current_settings = settings();
    let all_resolutions: Vec<(u32, u32, String)> = {
        let mut res: Vec<(u32, u32, String)> = RESOLUTIONS
            .iter()
            .map(|&(w, h, name)| (w, h, name.to_string()))
            .collect();

        // Add custom resolutions
        for custom in current_settings.custom_resolutions.iter() {
            res.push((custom.width, custom.height, custom.name.clone()));
        }
        res
    };

    // Check if current resolution is in the list
    let current_res_key = format!("{}x{}", current_settings.width, current_settings.height);

    let add_custom_resolution = move |_| {
        let w: u32 = custom_width().parse().unwrap_or(1920);
        let h: u32 = custom_height().parse().unwrap_or(1080);

        if w >= 640 && h >= 480 && w <= 7680 && h <= 4320 {
            let name = format!("{}x{} (Custom)", w, h);
            let custom = CustomResolution { width: w, height: h, name };

            // Check if already exists
            let exists = settings().custom_resolutions.iter().any(|r| r.width == w && r.height == h);
            if !exists {
                settings.write().custom_resolutions.push(custom);
                settings.write().width = w;
                settings.write().height = h;
                is_saved.set(false);
            }
        }
    };

    let window = use_window();
    let window_for_close = window.clone();
    let window_for_minimize = window.clone();
    let window_for_drag = window.clone();

    let hide_window = move |_| {
        window_for_close.set_visible(false);
    };

    let quit_app = move |_: dioxus::prelude::Event<dioxus::events::MouseData>| -> () {
        std::process::exit(0);
    };

    let minimize_window = move |_| {
        window_for_minimize.set_minimized(true);
    };

    let start_drag = move |_| {
        let _ = window_for_drag.drag_window();
    };

    rsx! {
        style { "{STYLE}" }

        div { class: "container",
            // Custom Titlebar - drag from anywhere
            div {
                class: "titlebar",
                onmousedown: start_drag,
                div {
                    class: "titlebar-left",
                    div {
                        class: "window-controls",
                        button {
                            class: "window-btn close",
                            onclick: hide_window,
                            title: "Hide"
                        }
                        button {
                            class: "window-btn minimize",
                            onclick: minimize_window,
                            title: "Minimize"
                        }
                    }
                    div { class: "titlebar-info",
                        span { class: "titlebar-title", "VDisplay" }
                        span { class: "titlebar-version", "v{version}" }
                    }
                }
                div {
                    class: "titlebar-status",
                    span { class: "{status_class}" }
                    span { "{status_text}" }
                }
            }

            // Scrollable Content
            div { class: "content",
                // Resolution Section
                div { class: "glass-card",
                    div { class: "section-header", "Display" }

                    // Horizontal row: Resolution + FPS
                    div { class: "form-row",
                        div { class: "form-group",
                            label { "Resolution" }
                            select {
                                value: "{current_res_key}",
                                onchange: move |e| {
                                    let value: String = e.value();
                                    let parts: Vec<&str> = value.split('x').collect();
                                    if parts.len() == 2 {
                                        if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
                                            settings.write().width = w;
                                            settings.write().height = h;
                                            is_saved.set(false);
                                            recreate_display_if_active();
                                        }
                                    }
                                },
                                for (w, h, name) in all_resolutions.iter() {
                                    option {
                                        key: "{w}x{h}",
                                        value: "{w}x{h}",
                                        selected: current_settings.width == *w && current_settings.height == *h,
                                        "{name}"
                                    }
                                }
                            }
                        }
                        div { class: "form-group",
                            label { "Refresh Rate" }
                            select {
                                value: "{settings().frame_rate}",
                                onchange: move |e| {
                                    if let Ok(fps) = e.value().parse() {
                                        settings.write().frame_rate = fps;
                                        is_saved.set(false);
                                        recreate_display_if_active();
                                    }
                                },
                                for &fps in FRAME_RATES.iter() {
                                    option {
                                        key: "{fps}",
                                        value: "{fps}",
                                        selected: settings().frame_rate == fps,
                                        "{fps} Hz"
                                    }
                                }
                            }
                        }
                    }

                    // Custom Resolution - compact horizontal
                    div { class: "form-group",
                        label { "Custom" }
                        div { class: "custom-resolution-row",
                            div { class: "input-group",
                                input {
                                    r#type: "number",
                                    value: "{custom_width}",
                                    placeholder: "W",
                                    oninput: move |e| custom_width.set(e.value())
                                }
                            }
                            span { class: "separator", "×" }
                            div { class: "input-group",
                                input {
                                    r#type: "number",
                                    value: "{custom_height}",
                                    placeholder: "H",
                                    oninput: move |e| custom_height.set(e.value())
                                }
                            }
                            button {
                                class: "btn btn-secondary btn-add",
                                onclick: add_custom_resolution,
                                "+"
                            }
                        }
                    }

                    // Custom Resolutions as chips
                    if !settings().custom_resolutions.is_empty() {
                        div { class: "custom-resolutions-list",
                            for (i, res) in settings().custom_resolutions.iter().enumerate() {
                                div { class: "custom-resolution-item",
                                    key: "{i}",
                                    span { "{res.width}×{res.height}" }
                                    button {
                                        class: "btn-remove",
                                        onclick: move |_| {
                                            settings.write().custom_resolutions.remove(i);
                                            is_saved.set(false);
                                        },
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }

                // Options Section - Toggle switches
                div { class: "glass-card",
                    div { class: "section-header", "Options" }

                    div { class: "toggle-row",
                        span { class: "toggle-label", "Auto-create on connect" }
                        label { class: "toggle-switch",
                            input {
                                r#type: "checkbox",
                                checked: settings().auto_create,
                                onchange: move |e| {
                                    settings.write().auto_create = e.checked();
                                    is_saved.set(false);
                                }
                            }
                            span { class: "toggle-slider" }
                        }
                    }

                    div { class: "toggle-row",
                        span { class: "toggle-label", "Start at login" }
                        label { class: "toggle-switch",
                            input {
                                r#type: "checkbox",
                                checked: settings().start_at_login,
                                onchange: move |e| {
                                    settings.write().start_at_login = e.checked();
                                    is_saved.set(false);
                                }
                            }
                            span { class: "toggle-slider" }
                        }
                    }
                }

                // Display controls
                div { class: "display-controls",
                    div { class: "display-status",
                        if display_active {
                            span { class: "status-indicator active" }
                            span { class: "status-text active", "Active" }
                        } else {
                            span { class: "status-indicator" }
                            span { class: "status-text", "Inactive" }
                        }
                    }
                div { class: "display-buttons",
                    button {
                        class: "btn btn-success",
                        disabled: display_active,
                        onclick: manual_create,
                        "Create Display"
                    }
                    button {
                        class: "btn btn-danger",
                        disabled: !display_active,
                        onclick: manual_destroy,
                        "Destroy Display"
                    }
                }
            }

            div { class: "buttons-row",
                button {
                    class: if is_saved() { "btn btn-primary saved" } else { "btn btn-primary" },
                    onclick: save_settings,
                    disabled: is_saved(),
                    if is_saved() { "Saved" } else { "Save Settings" }
                }
                button {
                    class: "btn btn-quit",
                    onclick: quit_app,
                    title: "Quit Application",
                    svg {
                        width: "14",
                        height: "14",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M18.36 6.64a9 9 0 1 1-12.73 0" }
                        line { x1: "12", y1: "2", x2: "12", y2: "12" }
                    }
                    span { "Quit" }
                }
            }
            }

            // Toast notification overlay (outside content, doesn't affect layout)
            if let Some((msg, success)) = toast_message() {
                div { class: "toast-container",
                    div {
                        class: if success { "toast success" } else { "toast error" },
                        "{msg}"
                    }
                }
            }
        }
    }
}
