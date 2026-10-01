-- m7-7.4 · 扩展 disposition_audit.action CHECK 约束，加入 'unlink'
--
-- 背景：M7-4 追加需求引入 `disp_unlink` 命令（仅对 hosting='external' 的资源），
-- 契约要求审计写入 action='unlink'。0003 迁移后的 CHECK 仅允许
-- ('archive', 'unarchive', 'soft_delete', 'destroy', 'undo_import')，
-- 直接 INSERT 'unlink' 会触发 CHECK constraint failed。
--
-- SQLite 不支持 ALTER CHECK 约束，需重建表：
--   1. 创建新表 disposition_audit_new（CHECK 加入 'unlink'）
--   2. 拷贝旧表数据
--   3. 删除旧表
--   4. 重命名新表
--   5. 重建索引
--
-- 无外键依赖（§6.7 disposition_audit 独立存活），重建安全。
-- 注意：sqlx::migrate 已在事务中执行，本文件不再显式 BEGIN/COMMIT。

CREATE TABLE disposition_audit_new (
    id               TEXT PRIMARY KEY,
    ref_id           TEXT,
    ref_name         TEXT,
    action           TEXT CHECK (action IN ('archive', 'unarchive', 'soft_delete', 'destroy', 'undo_import', 'unlink')),
    locator_snapshot TEXT,
    actor            TEXT,
    note             TEXT,
    at               INTEGER NOT NULL
);

INSERT INTO disposition_audit_new (id, ref_id, ref_name, action, locator_snapshot, actor, note, at)
SELECT id, ref_id, ref_name, action, locator_snapshot, actor, note, at FROM disposition_audit;

DROP TABLE disposition_audit;

ALTER TABLE disposition_audit_new RENAME TO disposition_audit;

CREATE INDEX IF NOT EXISTS idx_audit_ref ON disposition_audit(ref_id);
CREATE INDEX IF NOT EXISTS idx_audit_at  ON disposition_audit(at);
