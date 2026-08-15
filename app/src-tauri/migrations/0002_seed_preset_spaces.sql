-- 0002_seed_preset_spaces.sql
-- 预置空间："工作" / "生活"
-- id 固定以便测试（任务包 m2-2.1 §要求 4）
-- 时间戳使用固定 Unix 秒，保证迁移可重复执行（幂等）

INSERT INTO space (id, name, description, color, icon, status, created_at, updated_at)
SELECT
    'preset_space_work',
    '工作',
    '预置空间：工作',
    NULL,
    NULL,
    'active',
    1750000000,
    1750000000
WHERE NOT EXISTS (SELECT 1 FROM space WHERE id = 'preset_space_work');

INSERT INTO space (id, name, description, color, icon, status, created_at, updated_at)
SELECT
    'preset_space_life',
    '生活',
    '预置空间：生活',
    NULL,
    NULL,
    'active',
    1750000000,
    1750000000
WHERE NOT EXISTS (SELECT 1 FROM space WHERE id = 'preset_space_life');
