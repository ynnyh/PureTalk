use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

pub fn inject_text(text: &str) -> Result<(), String> {
    // Save old clipboard content
    let old_clipboard = Clipboard::new()
        .and_then(|mut c| c.get_text())
        .unwrap_or_default();

    // Set new text to clipboard
    let mut clipboard = Clipboard::new().map_err(|e| format!("访问剪贴板失败: {}", e))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("写入剪贴板失败: {}", e))?;

    thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("初始化输入模拟失败: {}", e))?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string())?;

    // Wait for paste to complete
    thread::sleep(Duration::from_millis(150));

    // Restore old clipboard
    let mut clipboard = Clipboard::new().map_err(|e| format!("访问剪贴板失败: {}", e))?;
    let _ = clipboard.set_text(old_clipboard);

    Ok(())
}
