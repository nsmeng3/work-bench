-- 0001_init.sql
-- 资源管理工作台 · 初始 schema
-- 依据《详细设计说明书》§3.2 表结构详表（10 张表）
-- 时间字段统一 INTEGER（Unix 秒），id 为 TEXT(UUID)

-- ============================================================
-- space
-- ============================================================
CREATE TABLE IF NOT EXISTS space (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(name) <= 64),
    description TEXT,
    color       TEXT,
    icon        TEXT,
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_space_status ON space(status);

-- ============================================================
-- collection
-- ============================================================
CREATE TABLE IF NOT EXISTS collection (
    id          TEXT PRIMARY KEY,
    space_id    TEXT NOT NULL REFERENCES space(id) ON DELETE RESTRICT,
    name        TEXT NOT NULL,
    summary     TEXT,
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_collection_space  ON collection(space_id);
CREATE INDEX IF NOT EXISTS idx_collection_status ON collection(status);

-- ============================================================
-- collection_tag
-- ============================================================
CREATE TABLE IF NOT EXISTS collection_tag (
    collection_id TEXT NOT NULL REFERENCES collection(id) ON DELETE CASCADE,
    tag           TEXT NOT NULL,
    PRIMARY KEY (collection_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_collection_tag_tag ON collection_tag(tag);

-- ============================================================
-- storage_source
-- ============================================================
CREATE TABLE IF NOT EXISTS storage_source (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('local_fs', 'git', 'cloud', 'object_storage')),
    is_default    INTEGER NOT NULL DEFAULT 0,
    config_json   TEXT,
    credential_ref TEXT,
    status        TEXT NOT NULL DEFAULT 'ok' CHECK (status IN ('ok', 'degraded', 'unavailable')),
    caps_json     TEXT
);

-- ============================================================
-- resource_reference
-- ============================================================
CREATE TABLE IF NOT EXISTS resource_reference (
    id              TEXT PRIMARY KEY,
    collection_id   TEXT NOT NULL REFERENCES collection(id) ON DELETE RESTRICT,
    source_id       TEXT NOT NULL REFERENCES storage_source(id),
    name            TEXT NOT NULL,
    type            TEXT NOT NULL CHECK (type IN ('code', 'document', 'data', 'artifact', 'tool', 'media')),
    hosting         TEXT NOT NULL CHECK (hosting IN ('external', 'managed')),
    locator_json    TEXT NOT NULL,
    description     TEXT,
    lifecycle       TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active', 'staged', 'delivered', 'archived')),
    confidentiality TEXT NOT NULL DEFAULT 'internal' CHECK (confidentiality IN ('public', 'internal', 'customer_restricted', 'sensitive')),
    indexed         INTEGER NOT NULL DEFAULT 1,
    disposition     TEXT NOT NULL DEFAULT 'none' CHECK (disposition IN ('none', 'archived', 'deleted')),
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ref_collection  ON resource_reference(collection_id);
CREATE INDEX IF NOT EXISTS idx_ref_type        ON resource_reference(type);
CREATE INDEX IF NOT EXISTS idx_ref_lifecycle   ON resource_reference(lifecycle);
CREATE INDEX IF NOT EXISTS idx_ref_disposition ON resource_reference(disposition);
CREATE INDEX IF NOT EXISTS idx_ref_locator_path ON resource_reference(json_extract(locator_json, '$.path'));

-- ============================================================
-- reference_tag
-- ============================================================
CREATE TABLE IF NOT EXISTS reference_tag (
    reference_id TEXT NOT NULL REFERENCES resource_reference(id) ON DELETE CASCADE,
    tag          TEXT NOT NULL,
    PRIMARY KEY (reference_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_reference_tag_tag ON reference_tag(tag);

-- ============================================================
-- watch_dir
-- ============================================================
CREATE TABLE IF NOT EXISTS watch_dir (
    id         TEXT PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    recursive  INTEGER NOT NULL DEFAULT 1,
    paused     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER
);

-- ============================================================
-- ignore_rule
-- ============================================================
CREATE TABLE IF NOT EXISTS ignore_rule (
    id         TEXT PRIMARY KEY,
    kind       TEXT CHECK (kind IN ('by_ext', 'by_name', 'by_dir')),
    value      TEXT NOT NULL,
    created_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ignore_kind ON ignore_rule(kind, value);

-- ============================================================
-- inbox_item
-- ============================================================
CREATE TABLE IF NOT EXISTS inbox_item (
    id             TEXT PRIMARY KEY,
    watch_dir_id   TEXT REFERENCES watch_dir(id) ON DELETE CASCADE,
    path           TEXT NOT NULL,
    event_kind     TEXT CHECK (event_kind IN ('created', 'modified', 'renamed', 'removed')),
    size_bytes     INTEGER,
    mtime          INTEGER,
    ext            TEXT,
    suggested_type TEXT,
    status         TEXT CHECK (status IN ('pending', 'snoozed', 'processed', 'ignored', 'stale')),
    assign_json    TEXT,
    ignore_rule_id TEXT REFERENCES ignore_rule(id),
    snooze_note    TEXT,
    remind_at      INTEGER,
    discovered_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_inbox_status ON inbox_item(status);
CREATE INDEX IF NOT EXISTS idx_inbox_path   ON inbox_item(path);
CREATE INDEX IF NOT EXISTS idx_inbox_watch  ON inbox_item(watch_dir_id);

-- ============================================================
-- disposition_audit (无外键，销毁后引用行已删除，审计独立存活 §6.7)
-- ============================================================
CREATE TABLE IF NOT EXISTS disposition_audit (
    id               TEXT PRIMARY KEY,
    ref_id           TEXT,
    ref_name         TEXT,
    action           TEXT CHECK (action IN ('archive', 'unarchive', 'soft_delete', 'destroy')),
    locator_snapshot TEXT,
    actor            TEXT,
    note             TEXT,
    at               INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_ref ON disposition_audit(ref_id);
CREATE INDEX IF NOT EXISTS idx_audit_at  ON disposition_audit(at);

-- ============================================================
-- settings
-- ============================================================
CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT,
    updated_at INTEGER
);

-- ============================================================
-- 初始数据：默认本地文件系统存储源（§3.2 storage_source 初始数据）
-- ============================================================
INSERT INTO storage_source (id, name, kind, is_default, config_json, credential_ref, status, caps_json)
SELECT
    'src_local_fs_default',
    'Local Filesystem',
    'local_fs',
    1,
    '{}',
    NULL,
    'ok',
    '{"archive":true,"softDelete":true,"destroy":true,"restoreFromBin":true,"retentionQuery":false}'
WHERE NOT EXISTS (SELECT 1 FROM storage_source WHERE id = 'src_local_fs_default');
