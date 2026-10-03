//! 状态栏驻留（系统托盘）：macOS 菜单栏 / Windows 通知区域 / Linux 托盘。
//!
//! - 精简菜单：「显示主窗口」「退出」。
//! - 左键点击托盘图标：切换主窗口显隐；右键（或 Ctrl+左键）弹出菜单。
//! - macOS 使用 template 图标，自动适配深/浅色菜单栏。
//!
//! 配合「后台运行」模式（lib.rs 拦截主窗口 CloseRequested 仅隐藏）：
//! 应用退出只走托盘「退出」菜单，触发 RunEvent::ExitRequested 做清理。

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// 托盘模板图标（44x44 黑形 + alpha，macOS template 模式自动着色）。
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray.png");

/// 显示并聚焦主窗口（托盘菜单 / 单实例二次启动共用）。
pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// 左键点击托盘：可见则隐藏，隐藏则显示并聚焦。
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => {
                let _ = w.hide();
            }
            _ => show_main_window(app),
        }
    }
}

/// 创建托盘图标与菜单（在 setup 中调用）；失败不阻塞应用启动。
pub fn setup_tray(app: &tauri::App) -> Result<(), String> {
    let show_item = MenuItemBuilder::with_id("show", "显示主窗口")
        .build(app)
        .map_err(|e| format!("构建托盘菜单项失败: {}", e))?;
    let quit_item = MenuItemBuilder::with_id("quit", "退出")
        .build(app)
        .map_err(|e| format!("构建托盘菜单项失败: {}", e))?;
    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()
        .map_err(|e| format!("构建托盘菜单失败: {}", e))?;

    let builder = TrayIconBuilder::new()
        .icon(Image::from_bytes(TRAY_ICON).map_err(|e| format!("解码托盘图标失败: {}", e))?)
        .menu(&menu)
        // 左键留给「切换窗口」，菜单用右键（macOS 上左键亦可 Ctrl+点击）
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        });

    // macOS：template 图标（仅 alpha 生效，自动适配菜单栏深/浅色）。
    // icon_as_template 为 macOS 专用 API，其它平台编译时跳过。
    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);

    builder
        .build(app)
        .map_err(|e| format!("创建托盘图标失败: {}", e))?;
    Ok(())
}
