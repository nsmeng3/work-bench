-- m7-7.1 · ref_access_log 表
--
-- 背景：M7-1 引入"资源打开/操作"埋点。每次前端成功调用
-- `ref_open` / `ref_reveal_in_finder` / 复制路径 / 用其他程序打开，
-- 都会写入一条 ref_access_log 记录，为 M7-3 Dashboard 的"最近资源"供数据。
--
-- action 取值：'open' | 'reveal' | 'copy_path' | 'open_with'
--   - open       ：默认打开（ref_open 不带 override）
--   - reveal     ：在文件管理器中显示（ref_reveal_in_finder）
--   - copy_path  ：复制路径到剪贴板（前端动作，仅埋点）
--   - open_with  ：用其他程序打开（ref_open 带 appOverride）
--
-- 外键：ref_id → resource_reference(id) ON DELETE CASCADE。
-- 与 disposition_audit 的"独立存活"策略不同：访问日志是行为数据，
-- 引用被销毁后保留意义不大，且会随时间膨胀，级联删除更合适。
--
-- 索引：
--   - idx_ref_access_at  ：按时间倒序查"最近访问"（M7-3 主查询）
--   - idx_ref_access_ref ：按引用查访问历史（详情面板"最近打开"扩展点）

CREATE TABLE IF NOT EXISTS ref_access_log (
    id        TEXT PRIMARY KEY,
    ref_id    TEXT NOT NULL REFERENCES resource_reference(id) ON DELETE CASCADE,
    action    TEXT CHECK (action IN ('open','reveal','copy_path','open_with')) NOT NULL,
    at        INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ref_access_at  ON ref_access_log(at DESC);
CREATE INDEX IF NOT EXISTS idx_ref_access_ref ON ref_access_log(ref_id);
