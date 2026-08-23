//! 应用入口：注册命令、初始化数据库连接池。

mod aggregate;
mod collection;
mod db;
mod disposition;
mod error;
mod fs_ops;
mod ignore;
mod inbox;
mod landing;
mod query;
mod reference;
mod sensitive;
mod settings;
mod space;
mod tag;
mod types;
mod watch;
mod dispatch;

use sqlx::sqlite::SqlitePool;

/// 全局应用状态：数据库连接池。
pub struct AppState {
    pub pool: SqlitePool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            // 数据库文件位置：{平台用户数据目录}/resource-workbench/workbench.db
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("获取 app data 目录失败: {}", e))?
                .join(db::APP_DATA_DIR);

            let pool = tauri::async_runtime::block_on(db::init_pool_at(&data_dir))
                .map_err(|e| format!("初始化数据库失败: {}", e))?;

            // 启动探测：系统回收站可用性 → 写入默认存储源 caps_json.softDelete（§5.1）。
            // 失败仅记录日志，不阻塞启动；下次启动会重试。
            let soft_delete_ok = disposition::detect_soft_delete_support();
            if let Err(e) = tauri::async_runtime::block_on(disposition::write_soft_delete_cap(
                &pool,
                "src_local_fs_default",
                soft_delete_ok,
            )) {
                eprintln!("[startup] 写入 caps_json.softDelete 失败: {}", e);
            }

            app.manage(AppState { pool });

            // m5-5.1 · 启动目录监听器（详细设计 §5.2）。
            // 事件通道先建 256 缓冲；上层消费者（5.2 忽略规则 / 5.3 聚合窗口）后续接入。
            // 启动失败仅记录日志，不阻塞应用启动。
            {
                let pool_for_watch = app.state::<AppState>().pool.clone();
                let (tx, rx) = tokio::sync::mpsc::channel::<watch::WatchEvent>(256);
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match watch::start_watchers(pool_for_watch.clone(), tx).await {
                        Ok(handle) => {
                            // manage 到 State，供 5.4 增量订阅 / 应用退出时 stop_watchers 使用。
                            app_handle.manage(handle);

                            // m5-5.2 · 忽略规则前置过滤：
                            // 加载规则（DB 用户规则 + 默认规则集），共享到 Arc<RwLock<..>>，
                            // 在 5.1 事件流与下游（5.3 聚合窗口）之间架过滤管道。
                            // 规则加载失败仅记录日志，退化为「无过滤」放行（不阻塞监听）。
                            let initial_rules = match ignore::load_rules(&pool_for_watch).await {
                                Ok(r) => r,
                                Err(e) => {
                                    eprintln!("[startup] 加载忽略规则失败，退化为无过滤: {}", e);
                                    Vec::new()
                                }
                            };
                            let shared_rules = std::sync::Arc::new(
                                std::sync::RwLock::new(initial_rules),
                            );
                            // manage 到 State，供 5.4 inbox_ignore 触发规则重载时写入。
                            app_handle.manage(shared_rules.clone());

                            // 过滤后的事件流出口：5.3 聚合窗口消费 filtered_rx。
                            let (filtered_tx, filtered_rx) =
                                tokio::sync::mpsc::channel::<watch::WatchEvent>(256);
                            let _filter_handle =
                                ignore::filter_events(rx, filtered_tx, shared_rules);
                            // _filter_handle 在后台持续运行；应用退出时随 runtime 关闭。

                            // m5-5.3 · 聚合窗口：消费 5.2 过滤后的事件流，
                            // 按 5 秒窗口缓冲合并写入 inbox_item。
                            let _aggregator_handle = aggregate::start_aggregator(
                                pool_for_watch.clone(),
                                filtered_rx,
                            );
                            // _aggregator_handle 在后台持续运行；应用退出时随 runtime 关闭。
                        }
                        Err(e) => {
                            eprintln!("[startup] 启动目录监听失败: {}", e);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            space::space_create,
            space::space_update,
            space::space_archive,
            space::space_restore,
            space::space_list,
            collection::collection_create,
            collection::collection_update,
            collection::collection_archive,
            collection::collection_restore,
            collection::collection_get,
            collection::collection_list,
            reference::ref_create_external,
            reference::ref_create_managed,
            reference::ref_update,
            reference::ref_get,
            reference::ref_list,
            tag::tag_add,
            tag::tag_remove,
            tag::tag_list,
            tag::tag_query,
            query::query_refs,
            query::query_facets,
            fs_ops::ref_check_health,
            fs_ops::ref_open,
            fs_ops::ref_reveal_in_finder,
            settings::settings_get_root_dir,
            settings::settings_init_root_dir,
            settings::settings_get_default_app,
            settings::settings_set_default_app,
            disposition::disp_get_capabilities,
            settings::settings_change_root_dir,
            disposition::disp_archive,
            disposition::disp_unarchive,
            disposition::disp_preview,
            disposition::disp_preview_cancel,
            disposition::disp_destroy,
            disposition::disp_soft_delete,
            disposition::disp_audit_list,
            disposition::ref_undo_import,
            inbox::inbox_list,
            inbox::inbox_get,
            inbox::inbox_assign,
            inbox::inbox_snooze,
            inbox::inbox_ignore,
            inbox::inbox_dismiss_stale,
            inbox::inbox_stats,
            inbox::watch_dir_get,
            inbox::watch_dir_set,
            inbox::watch_dir_unset,
            dispatch::watch_dir_event,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
