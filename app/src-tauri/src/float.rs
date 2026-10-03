//! 悬浮通知窗：应用统一的桌面通知通道（约定见 CLAUDE.md）。
//!
//! 无边框、透明、置顶、不占任务栏的小窗，启动时创建（隐藏），
//! 保证前端 JS 事件监听提前就绪，首次通知不丢数据。
//!
//! 命令（供前端 invoke）：
//! - `float_notify { title, body, action? }`：定位到主窗口所在显示器右上角
//!   → 显示 → emit `float-data`。`action` 为可选的「点击后向主窗口 emit 的事件名」，
//!   例如收件箱通知传 "go-inbox"；不传则点击仅唤起主窗口。
//! - `float_hide`：隐藏（前端倒计时结束或点关闭按钮时调用，幂等）。
//! - `float_click { action? }`：悬浮卡片被点击 → 隐藏自己、唤起并聚焦主窗口，
//!   并按 action 向主窗口 emit 对应事件。
//!
//! 窗口控制全部在 Rust 侧完成，前端无需额外的 window 权限。

use tauri::{Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

/// 悬浮窗 label；前端 URL 带 `?float=1` 时渲染 FloatApp 而非主界面
pub const FLOAT_LABEL: &str = "float";

const FLOAT_WIDTH: f64 = 340.0;
const FLOAT_HEIGHT: f64 = 92.0;
const MARGIN_RIGHT: f64 = 16.0;
const MARGIN_TOP: f64 = 16.0;

/// `float_notify` 的事件负载（对齐前端 FloatApp 的渲染字段）。
#[derive(serde::Serialize, Clone)]
struct FloatPayload {
    /// 通知标题（如「目录监控」）。
    title: String,
    /// 通知正文（单行，超长由前端省略）。
    body: String,
    /// 点击卡片后向主窗口 emit 的事件名；None 表示点击仅唤起主窗口。
    action: Option<String>,
}

/// 启动时创建隐藏的悬浮窗（在 setup 中调用）。
/// 失败不阻塞应用启动，由调用方降级处理。
pub fn setup_float_window(app: &tauri::App) -> Result<(), String> {
    WebviewWindowBuilder::new(
        app,
        FLOAT_LABEL,
        WebviewUrl::App("index.html?float=1".into()),
    )
    .title("通知")
    .inner_size(FLOAT_WIDTH, FLOAT_HEIGHT)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .build()
    .map_err(|e| format!("创建悬浮窗失败: {}", e))?;
    Ok(())
}

/// 显示悬浮窗并投递通知内容：定位到「主窗口所在显示器」右上角（回退主屏）。
#[tauri::command]
pub fn float_notify(
    app: tauri::AppHandle,
    title: String,
    body: String,
    action: Option<String>,
) -> Result<(), String> {
    let float_win = app
        .get_webview_window(FLOAT_LABEL)
        .ok_or_else(|| "悬浮窗不存在".to_string())?;

    let monitor = app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    if let Some(m) = monitor {
        // 物理像素 → 逻辑像素
        let scale = m.scale_factor();
        let origin_x = m.position().x as f64 / scale;
        let origin_y = m.position().y as f64 / scale;
        let logical_w = m.size().width as f64 / scale;
        let x = origin_x + logical_w - FLOAT_WIDTH - MARGIN_RIGHT;
        let y = origin_y + MARGIN_TOP;
        let _ = float_win.set_position(LogicalPosition::new(x, y));
    }

    float_win
        .show()
        .map_err(|e| format!("显示悬浮窗失败: {}", e))?;
    float_win
        .emit("float-data", FloatPayload { title, body, action })
        .map_err(|e| format!("悬浮窗事件投递失败: {}", e))?;
    Ok(())
}

/// 隐藏悬浮窗（幂等）。
#[tauri::command]
pub fn float_hide(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(FLOAT_LABEL) {
        let _ = w.hide();
    }
    Ok(())
}

/// 点击悬浮卡片：隐藏自己，唤起主窗口并按 action emit 对应事件（如有）。
#[tauri::command]
pub fn float_click(app: tauri::AppHandle, action: Option<String>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(FLOAT_LABEL) {
        let _ = w.hide();
    }
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.show();
        let _ = main_win.unminimize();
        let _ = main_win.set_focus();
        if let Some(event) = action {
            main_win
                .emit(&event, ())
                .map_err(|e| format!("悬浮窗点击事件投递失败 ({}): {}", event, e))?;
        }
    }
    Ok(())
}
