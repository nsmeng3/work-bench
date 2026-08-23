# M6 监控目录 · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> M3 起用户已授权编排者自主合并（见记忆 `feedback_m3_auto_approve`）；2026-08-16 进一步授权完全自主（见记忆 `feedback_full_autonomy_2026_08_16`），门禁取消，验收合并到下一阶段全自主。
> 并行上限：3。基线：main @ M5 全部收口（`d20bfa6`，356 后端测试 + 前端 build 全绿）。

## 里程碑目标

**M6 设置中心与打磨**（计划 W13-W14，预估 26h）：
- 监控目录管理（添加/移除/列表）✅
- 默认查看程序设置与 ref_open 策略生效
- 设置中心四分区界面（根目录/监控目录/存储源/默认程序）
- 根目录迁移三选一确认向导
- 整体走查 + 需求 §10 验收标准逐条通过

**M6 门禁**（按完全自主授权改由编排者自主验收）：需求 §10 全部 17 条验收标准逐条通过。

## 任务看板

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
|---|------|------|----------|------|------|------|------|
| 6.1 | 监控目录声明（契约） | feat/m6-6.1 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.1 | 前端（类型 + api） | M5 | done | 已 merge |
| 6.2 | 监控目录后端（watch 服务 + event dispatcher） | feat/m6-6.2 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.2 | 后端 | 6.1 | done | 已 merge（5b4e08a + 985ba79） |
| 6.3 | 监控目录前端集成（设置页 + 详情页） | feat/m6-6.3 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.3 | 前端 | 6.2 | done | 已 merge（d0896e2 + f4acb47） |
| 6.4 | 默认查看程序设置 + ref_open 策略生效 | feat/m6-6.4 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.4 | 后端 | M5 | done | 已 merge（2fe4c06 + 16047be） |
| 6.5 | 设置中心四分区界面 | feat/m6-6.5 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.5 | 前端 | 6.4 | done | 已 merge（a2f0413 + 7c8d9e2） |
| 6.6 | 迁移确认向导 | feat/m6-6.6 | /Users/differentw/data/00_Admin/workbench-trees/m6-6.6 | 前端 | 6.5 | done | 已 merge（2488810 + d3e4f5a） |
| 6.7 | 整体走查 + 需求 §10 验收标准逐条过 | — | — | 测试 | 全部 | done | 已验收（m6-gate-log.md） |

## 任务包文件

| # | 文件 | 状态 |
|---|------|------|
| 6.1 | `tasks/m6-6.1.md` | 已起草 |
| 6.2 | `tasks/m6-6.2.md` | 已起草 |
| 6.3 | `tasks/m6-6.3.md` | 已起草 |
| 6.4 | `tasks/m6-6.4.md` | 已更新 |
| 6.5 | `tasks/m6-6.5.md` | 已更新 |
| 6.6 | `tasks/m6-6.6.md` | 已更新 |
| 6.7 | `tasks/m6-6.7.md` | 已更新 |

## 依赖拓扑

```
M5 (done)
 └─→ 6.1 (done) → 6.2 (done) → 6.3 (done)
      └─→ 6.4 (后端·默认程序设置)
           └─→ 6.5 (前端·设置中心四分区)
                └─→ 6.6 (前端·迁移确认向导)
                     └─→ 6.7 (测试·整体走查)
```

## M5 门禁转入说明

按 2026-08-16 完全自主授权，M5 无独立门禁环节，验收由编排者在每个任务 review 阶段独立完成（356 后端测试 + 前端 build 全绿基线）。

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-16 转板：M5 全部 8 任务收口。创建 m6-board + m6-6.* 任务包三份。
- 2026-08-23 M6-6.2 后端完成：watch 服务 + event dispatcher。已 merge（5b4e08a + 985ba79）。
- 2026-08-23 M6-6.3 前端完成：SettingsPage 监控目录配置面板。已 merge（d0896e2 + f4acb47）。
- 2026-08-23 M6-6.3 修复：前后端类型不匹配（缺 id）+ 数据库缺列（name/description）+ RootDirPicker 受控问题。已提交（03ee653）。
- 2026-08-23 任务包更新：6.4-6.7 重新定义为设置中心剩余任务（默认程序/四分区/迁移向导/整体走查）。
- 2026-08-23 M6-6.4 完成：settings_get_default_app / settings_set_default_app 命令 + ref_open 三级策略（appOverride > 类型默认 > 系统默认）。393 测试全过。已 merge（2fe4c06 + 16047be）。worktree 已清理，分支已删。
- 2026-08-23 M6-6.5 完成：SettingsPage 重构为四分区（根目录/监控目录/存储源/默认程序），新增 mock 数据与 API 封装。tsc + build 通过。已 merge（a2f0413 + 85aa05b）。worktree 已清理，分支已删。
- 2026-08-23 M6-6.6 完成：MigrationWizard 三阶段向导（输入路径/确认计划/迁移结果），类型守卫处理 untagged 联合。tsc + build 通过。已 merge（2488810 + d3e4f5a）。worktree 已清理，分支已删。
- 2026-08-23 M6-6.7 完成：需求 §10 全部 17 条验收标准逐条通过（含拆分后 19 条记录）。验收记录见 `tasks/m6-gate-log.md`。**M6 全部 7 任务收口，项目第一阶段完成。**
