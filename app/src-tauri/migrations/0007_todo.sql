-- m7-7.2 · todo + todo_ref_link
--
-- 背景：M7-2 引入"待办"作为固定入口，让用户记录待办事项。
-- 每条 todo 可挂到一个项目（space_id，可空表示全局 todo），
-- 可挂载多个资源引用（多对多，通过 todo_ref_link）。
--
-- 关键设计（与用户对齐）：
--   - todo 挂 space_id，不挂 collection_id（项目=空间，太细的粒度反而乱）
--   - space_id 可空（"回邮件"这种全局 todo）
--   - 状态机四态：pending / doing / done / cancelled
--   - 优先级 0/1/2（普通/重要/紧急）
--   - 不支持子任务（M8 再说）
--   - sort_order 预留但本任务不做 UI 拖拽
--
-- 外键：
--   - todo.space_id → space(id) ON DELETE SET NULL
--     （删除空间不丢 todo，仅置空；与用户确认）
--   - todo_ref_link.todo_id → todo(id) ON DELETE CASCADE
--   - todo_ref_link.ref_id   → resource_reference(id) ON DELETE CASCADE
--     （引用被销毁后挂载关系无保留意义，级联删除）
--
-- 索引：
--   - idx_todo_status      ：按状态过滤（默认列表排除 done/cancelled）
--   - idx_todo_space       ：空间详情页"待办"tab 按 space_id 过滤
--   - idx_todo_due         ：按截止时间排序（M8 提醒扩展点）
--   - idx_todo_ref_link_ref：资源详情页"被哪些 todo 引用"反查

CREATE TABLE IF NOT EXISTS todo (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL CHECK (length(title) >= 1 AND length(title) <= 200),
    note        TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending','doing','done','cancelled')),
    space_id    TEXT REFERENCES space(id) ON DELETE SET NULL,
    priority    INTEGER NOT NULL DEFAULT 0
                CHECK (priority IN (0,1,2)),  -- 0 普通 1 重要 2 紧急
    due_at      INTEGER,                       -- Unix 秒；NULL = 无截止
    done_at     INTEGER,                       -- status 变为 done 时记录
    sort_order  INTEGER NOT NULL DEFAULT 0,    -- 手动排序（预留）
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_todo_status ON todo(status);
CREATE INDEX IF NOT EXISTS idx_todo_space  ON todo(space_id);
CREATE INDEX IF NOT EXISTS idx_todo_due    ON todo(due_at);

CREATE TABLE IF NOT EXISTS todo_ref_link (
    todo_id    TEXT NOT NULL REFERENCES todo(id) ON DELETE CASCADE,
    ref_id     TEXT NOT NULL REFERENCES resource_reference(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (todo_id, ref_id)
);
CREATE INDEX IF NOT EXISTS idx_todo_ref_link_ref ON todo_ref_link(ref_id);
