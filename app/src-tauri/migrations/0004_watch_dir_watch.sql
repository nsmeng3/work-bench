-- m6-6.2 · watch_dir_events：事件缓冲去重 + 聚合窗口
-- 分区键：(dir_id, created)
-- 唯一键：(dir_id, path, kind, created) → 同路径同文件同时间戳只记一次
-- 索引：idx_watch_dir_events_dir_id, idx_watch_dir_events_kind, idx_watch_dir_events_path

CREATE TABLE IF NOT EXISTS watch_dir_events (
    id                  TEXT PRIMARY KEY,
    dir_id              TEXT NOT NULL REFERENCES watch_dir(id) ON DELETE CASCADE,
    path                TEXT NOT NULL,
    kind                TEXT NOT NULL CHECK (kind IN ('created', 'modified', 'renamed', 'removed')),
    created_at          INTEGER NOT NULL,
    processed_at        INTEGER,
    FOREIGN KEY (dir_id) REFERENCES watch_dir(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_watch_dir_events_dir_id ON watch_dir_events(dir_id);
CREATE INDEX IF NOT EXISTS idx_watch_dir_events_kind ON watch_dir_events(kind);
CREATE INDEX IF NOT EXISTS idx_watch_dir_events_path ON watch_dir_events(path);
