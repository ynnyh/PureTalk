use serde::Serialize;
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorState {
    pub state: String, // "recording" | "transcribing" | "done" | "idle"
}

/// Show the indicator window without stealing focus from the active application.
/// On Windows, we set WS_EX_NOACTIVATE to prevent focus stealing.
pub fn show_indicator(app: &tauri::AppHandle, state: &str) {
    let event_state = IndicatorState {
        state: state.to_string(),
    };

    if let Some(window) = app.get_webview_window("indicator") {
        let _ = window.emit("indicator:state", &event_state);
        show_no_activate(&window);
    } else {
        let window = tauri::WebviewWindowBuilder::new(
            app,
            "indicator",
            tauri::WebviewUrl::App("src/windows/indicator/index.html".into()),
        )
        .title("")
        .inner_size(360.0, 64.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .shadow(false)
        .visible(false)
        .build()
        .expect("failed to build indicator window");

        // Position at bottom center of screen
        if let Ok(monitor) = window.current_monitor() {
            if let Some(monitor) = monitor {
                let screen_size = monitor.size();
                let scale = monitor.scale_factor();
                let win_w = 360.0;
                let win_h = 64.0;
                let x = (screen_size.width as f64 / scale - win_w) / 2.0;
                let y = screen_size.height as f64 / scale - win_h - 60.0;
                let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
            }
        }

        // Apply WS_EX_NOACTIVATE on Windows to prevent focus stealing
        #[cfg(windows)]
        set_no_activate(&window);

        // Show without activating
        show_no_activate(&window);

        // Emit after a small delay to ensure the frontend is ready
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            if let Some(w) = handle.get_webview_window("indicator") {
                let _ = w.emit("indicator:state", &event_state);
            }
        });
    }
}

/// Show window without activating (stealing focus).
#[cfg(windows)]
fn show_no_activate(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = ShowWindow(hwnd.0, SW_SHOWNOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
fn show_no_activate(window: &tauri::WebviewWindow) {
    let _ = window.show();
}

/// Set WS_EX_NOACTIVATE on the window to prevent it from stealing focus on Windows.
#[cfg(windows)]
fn set_no_activate(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let ex_style = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
            let _ = SetWindowLongPtrW(hwnd.0, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE as isize);
        }
    }
}

pub fn hide_indicator(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("indicator") {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
            if let Ok(hwnd) = window.hwnd() {
                unsafe {
                    let _ = ShowWindow(hwnd.0, SW_HIDE);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = window.hide();
        }
    }
}

pub fn show_done_then_hide(app: &tauri::AppHandle) {
    show_indicator(app, "done");
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
        hide_indicator(&handle);
    });
}
