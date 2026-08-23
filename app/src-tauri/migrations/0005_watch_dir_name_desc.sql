-- m6-6.3 · watch_dir 表扩展：name + description
-- 监控目录配置面板需要名称和描述字段

ALTER TABLE watch_dir ADD COLUMN name TEXT;
ALTER TABLE watch_dir ADD COLUMN description TEXT;
