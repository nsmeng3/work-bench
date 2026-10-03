-- m8-8.5 · todo 支持挂到资源集（collection）粒度
--
-- 背景：M7-2 的 todo 只能挂 space（空间级）。M8-5 将归属粒度细化到
-- collection（资源集）：collection_id 可空，NULL 表示不属于任何资源集。
--
-- 约束与级联：
-- - REFERENCES collection(id) ON DELETE SET NULL：资源集删除时 todo 降级
--   为无归属（与 space_id 的 ON DELETE SET NULL 策略一致，不丢数据）。
-- - 不约束 collection_id 与 space_id 一致（允许只挂资源集不挂空间，
--   也允许两者都挂；一致性由前端选择器自然保证）。

ALTER TABLE todo ADD COLUMN collection_id TEXT REFERENCES collection(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_todo_collection ON todo(collection_id);
