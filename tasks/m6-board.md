# M6 监控目录 · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> M3 起用户已授权编排者自主合并（见记忆 `feedback_m3_auto_approve`）；2026-08-16 进一步授权完全自主（见记忆 `feedback_full_autonomy_2026_08_16`），门禁取消，验收合并到下一阶段全自主。
> 并行上限：3。基线：main @ M5 全部收口（`d20bfa6`，356 后端测试 + 前端 build 全绿）。

## 里程碑目标

**M6 监控目录**（计划 W13，预估 24h）：
- 添加监控目录后新建文件能进收件箱
- 监控目录列表 / 详情 / 移除可见
- 事件缓冲聚合窗口对齐 M5

**M6 门禁**（按完全自主授权改由编排者自主验收）：添加一个监控目录 → 新增若干文件 → 收件箱正确聚合、敏感提示 → 处理为引用 → 列表/详情界面可见 → 移除目录并清理事件缓冲。

## 任务看板

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
|---|------|------|----------|------|------|------|------|
| 6.1 | 监控目录声明（契约） | feat/m6-6.1 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.1 | 前端（类型 + api） | M5 | done | 已 merge（4e41e45 之后的下一个 commit） |
| 6.2 | 监控目录后端（watch 服务 + event dispatcher） | feat/m6-6.2 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.2 | 后端 | 6.1 | done | 已 merge（5b4e08a + 985ba79） |
| 6.3 | 监控目录前端集成（设置页 + 详情页） | feat/m6-6.3 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.3 | 前端 | 6.2 | done | 已 merge（d0896e2 + f4acb47） |

## 任务包文件

| # | 文件 | 状态 |
|---|------|------|
| 6.1 | `tasks/m6-6.1.md` | 已起草 |
| 6.2 | `tasks/m6-6.2.md` | 已起草 |
| 6.3 | `tasks/m6-6.3.md` | 已起草 |

## M5 门禁转入说明

按 2026-08-16 完全自主授权，M5 无独立门禁环节，验收由编排者在每个任务 review 阶段独立完成（356 后端测试 + 前端 build 全绿基线）。

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-16 转板：M5 全部 8 任务收口（4e41e45 分支已 merge）。创建 m6-board + m6-6.* 任务包三份（契约已起草，后续再补 detail）。当前并行度 0/3。
- 2026-08-23 M6-6.2 后端完成：创建 dispatch.rs 实现 watch_dir_event 命令，创建 0004_watch_dir_watch.sql 迁移文件，注册命令到 lib.rs，单元测试通过。已 merge（5b4e08a + 985ba79）。
- 2026-08-23 M6-6.3 前端完成：创建 SettingsPage.tsx 实现监控目录配置面板，启用设置导航项，添加/移除目录功能。已 merge（d0896e2 + f4acb47）。M6 全部 3 任务收口。
