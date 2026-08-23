# M5 收件箱 · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> M3 起用户已授权编排者自主合并（见记忆 `feedback_m3_auto_approve`）；2026-08-16 进一步授权完全自主（见记忆 `feedback_full_autonomy_2026_08_16`），门禁取消，验收合并到下一阶段全自主。
> 并行上限：3。基线：main @ M4 全部收口（`cd2e811`，273 后端测试 + 前端 build 全绿）。

## 里程碑目标

**M5 收件箱**（计划 W10-W12，预估 36h）：
- 添加监控目录后新建文件能进收件箱
- 聚合通知、去重、四类处理可用（仅关联 / 导入并托管 / 暂后 / 忽略）
- 敏感文件识别与预览保护

**M5 门禁（按完全自主授权改由编排者自主验收）**：添加一个监控目录 → 拖入若干文件（含敏感文件与构建产物）→ 收件箱正确聚合、过滤、提示 → 逐条处理为正式引用。

## 任务看板

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 5.1 | 目录监听组件（notify 接入 + 事件标准化） | feat/m5-5.1-watch | /Users/differentw/data/00_Admin/workbench-trees/m5-5.1 | 后端 | M4 | done | 已合并 main（cb43e10，--no-ff），worktree 已清理，分支已删 |
| 5.2 | 忽略规则引擎（默认规则 + 用户规则匹配） | feat/m5-5.2-ignore-rules | /Users/differentw/data/00_Admin/workbench-trees/m5-5.2 | 后端 | 5.1 | done | 已合并 main（a6277ae，--no-ff），worktree 已清理，分支已删 |
| 5.3 | 聚合窗口缓冲 + 去重（路径查重、已有关联查重） | feat/m5-5.3-aggregate | /Users/differentw/data/00_Admin/workbench-trees/m5-5.3 | 后端 | 5.2 | done | 已合并 main（36127a6，--no-ff），worktree 已清理，分支已删 |
| 5.4 | 收件箱服务：7 个命令 | feat/m5-5.4-inbox-svc | /Users/differentw/data/00_Admin/workbench-trees/m5-5.4 | 后端 | 5.3 | done | 已合并 main（415b788，--no-ff），worktree 已清理，分支已删；schema 偏差已折中（processedAt 写入 assign_json 快照而非顶层字段） |
| 5.5 | 敏感文件识别与预览保护 | feat/m5-5.5-sensitive | /Users/differentw/data/00_Admin/workbench-trees/m5-5.5 | 后端 | 5.4 | done | 已合并 main（5e7320f，--no-ff），worktree 已清理，分支已删 |
| 5.6 | 前端：收件箱列表 + 条目详情 | feat/m5-5.6-inbox-ui | /Users/differentw/data/00_Admin/workbench-trees/m5-5.6 | 前端 | 5.4 | done | 已合并 main（b0ec0e0，--no-ff），worktree 已清理，分支已删 |
| 5.7 | 前端：处理对话框（选空间/资源集/类型 + 仅关联或托管） | feat/m5-5.7-assign-ui | /Users/differentw/data/00_Admin/workbench-trees/m5-5.7 | 前端 | 5.6 | done | 已合并 main（ac0086e，mock.ts 冲突保留双方 + 手工合并 mockInboxApi 结构），worktree 已清理，分支已删 |
| 5.8 | 前端：合并通知与角标 | feat/m5-5.8-notify-ui | /Users/differentw/data/00_Admin/workbench-trees/m5-5.8 | 前端 | 5.6 | done | 已合并 main（d20bfa6，--no-ff），worktree 已清理，分支已删 |

## 任务包文件

| # | 文件 | 状态 |
| --- | --- | --- |
| 5.1 | `tasks/m5-5.1.md` | 已起草 |
| 5.2 | `tasks/m5-5.2.md` | 已起草 |
| 5.3 | `tasks/m5-5.3.md` | 已起草 |
| 5.4 | `tasks/m5-5.4.md` | 已起草 |
| 5.5 | `tasks/m5-5.5.md` | 已起草 |
| 5.6 | `tasks/m5-5.6.md` | 已起草 |
| 5.7 | `tasks/m5-5.7.md` | 已起草 |
| 5.8 | `tasks/m5-5.8.md` | 已起草 |

## 依赖拓扑

```
M4 (done)
 └─→ 5.1 (后端·notify 接入)
      └─→ 5.2 (后端·忽略规则)
           └─→ 5.3 (后端·聚合窗口)
                └─→ 5.4 (后端·7 命令)
                     ├─→ 5.5 (后端·敏感文件)
                     ├─→ 5.6 (前端·收件箱列表)
                     │    ├─→ 5.7 (前端·处理对话框)
                     │    └─→ 5.8 (前端·合并通知角标)
                     └─→ ...
```

**并行提示**：
- 严格串行链：5.1→5.2→5.3→5.4（后端基础设施必须按序）
- 5.4 入库后可并行：5.5 + 5.6
- 5.6 入库后可并行：5.7 + 5.8
- 单线推进时并行度低，可在 5.2/5.3 阶段补 M4 收尾/清理工作

## 共享资源号段分配（防撞车）

| 资源 | 分配规则 |
| --- | --- |
| sqlx migration | M5 默认**不需新表**：`watch_dir` / `inbox_item` 表 M1 `0001_init.sql:96,120` 已建好；忽略规则若需新表（`ignore_rule`）须先向用户申请并登记号段。**注意 M4-4.9 已破例新增 0003（audit action CHECK 扩展），后续 migration 从 0004 起** |
| `pnpm-lock.yaml` | 5.1 需要新增 Rust 依赖 `notify` crate；先看板备注登记，编排者统一在 main 安装后各分支 rebase |
| 接口契约（详细设计 §2.7/§5.2） | `inbox_*` 7 命令 + `settings_*_watch_dir` 4 命令契约已冻结；事件标准化字段（created/modified/renamed/removed）在 §5.2 冻结 |
| vite dev 端口 | 验收时一次只跑一个实例，不允许并行起 dev server |
| 前端 UI 库 | **Ant Design v5**（M2 决策延续） |
| 聚合窗口参数 | 详细设计 §5.2：5 秒聚合窗口 + 同路径去重；5.3 实现时固化为常量 |

## M4 门禁转入说明

按 2026-08-16 完全自主授权，M4 无独立门禁环节，验收由编排者在每个任务 review 阶段独立完成（273 后端测试 + 前端 build 全绿基线）。

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-16 初始化：M4 全部 9 任务收口（含 2 backlog），M5 看板建立（8 任务）。任务包文件待起草，等用户确认范围后逐条落 `tasks/m5-5.*.md` 并启动第一波（5.1）。
- 2026-08-16 任务包起草完成：8 份 `tasks/m5-5.*.md` 落地，契约锚点、范围清单、验收命令齐备。待用户确认后启动第一波（5.1 目录监听组件）。
- 2026-08-16 第 1 轮（用户启动 M5）：5.1 已派出，worktree 已建，Agent 首步自建索引。当前并行度 1/3（M5 严格串行链 5.1→5.2→5.3→5.4，前期并行度低）。
- 2026-08-16 第 2 轮（Agent 通知触发）：5.1 验收通过 → 合并（自主验收：范围 4 文件干净、284 测试全过含 11 个 watch 新测试 + 2 个 macOS 集成测试本机跑通、notify 8.2.0 已接入含版本注释、WatcherHandle manage 到 State）。main@cb43e10 验证全绿。worktree 已清理，分支已删。**依赖已解锁：5.2 可派出。**
- 2026-08-16 第 3 轮（自动派出）：5.2（忽略规则引擎）。当前并行度 1/3。
- 2026-08-16 第 4 轮（Agent 通知触发）：5.2 验收通过 → 合并（自主验收：范围 4 文件干净、306 测试全过含 22 个 ignore 新测试、glob 0.3.4 已登记接入、filter_events 管道接入 watch.rs 零侵入、Arc<RwLock> 共享规则集 manage 到 State 供 5.4 重载）。main@a6277ae 验证全绿。worktree 已清理，分支已删。**依赖已解锁：5.3 可派出。**
- 2026-08-16 第 5 轮（自动派出）：5.3（聚合窗口 + 去重）。当前并行度 1/3。
- 2026-08-16 第 6 轮（Agent 通知触发）：5.3 验收通过 → 合并（自主验收：范围 2 文件干净、314 测试全过含 8 个 aggregate 新测试、AGGREGATE_WINDOW_SECS=5 固化、接入点严格在 ignore::filter_events 之后零侵入）。main@36127a6 验证全绿。worktree 已清理，分支已删。**依赖已解锁：5.4 可派出。**
- 2026-08-16 第 7 轮（自动派出）：5.4（收件箱 7 命令，集成 M2 create_external + M3 create_managed）。当前并行度 1/3。
- 2026-08-16 第 8 轮（Agent 通知触发）：5.4 验收通过 → 合并（自主验收：范围 2 文件干净、330 测试全过含 18 个 inbox 新测试、7 命令全部注册、inbox_assign 集成 M2/M3 既有函数仅调用未修改、inbox_ignore 触发 5.2 规则热更新、schema 偏差折中 processedAt 写入 assign_json 快照）。main@415b788 验证全绿。worktree 已清理，分支已删。**依赖已解锁：5.5 + 5.6 可派出。**
- 2026-08-16 第 9 轮（用户批准筛选页跳转建议）：用户提出「筛选中的引用列表应该支持跳转到资源所在的资源集」，批准立即派 Agent 修复（小改、独立、不影响 M5）。与 5.5/5.6 并行派出。
- 2026-08-16 第 10 轮（三路并行派出）：5.5（敏感文件识别）+ 5.6（收件箱列表 UI）+ 筛选页跳转修复（feat/m5-filter-jump，单文件小改）。当前并行度 3/3。
- 2026-08-16 第 11 轮（用户优化建议）：空间列表去掉「进入」按钮改为点击条目进入。派出 feat/m5-space-click（单文件小改，独立无冲突）。当前并行度临时 4/3（小改很快）。
- 2026-08-16 第 12 轮（Agent 通知触发）：筛选页跳转修复验收通过 → 合并（自主验收：范围 2 文件干净、build 通过、App.tsx 注入 handleJumpToCollection 回调、FilterPage 加跳转列、按钮 disabled 条件防误点）。main@9be3e9e 验证全绿（330 测试 + build）。worktree 已清理，分支已删。仍在跑：5.5 / 5.6 / 空间列表优化。
- 2026-08-16 第 13 轮（Agent 通知触发）：空间列表优化验收通过 → 合并（自主验收：范围 1 文件干净、build 通过、SpacePage 整行 onClick + onRow cursor pointer + 操作列按钮 stopPropagation）。main@c237249 验证全绿。worktree 已清理，分支已删。仍在跑：5.5 / 5.6。
- 2026-08-16 第 14 轮（Agent 通知触发）：5.5 验收通过 → 合并（自主验收：范围 3 文件干净、356 测试全过含 28 个 sensitive 新测试 + 24 个 inbox 测试、SENSITIVE_PATTERNS 硬编码规则、inbox_get 集成敏感检查 preview=None + sensitiveWarning 非空）。main@5e7320f 验证全绿。worktree 已清理，分支已删。仍在跑：5.6。
- 2026-08-16 第 15 轮（用户优化建议）：资源集列表去掉「详情」按钮改为点击条目进入。派出 feat/m5-coll-click（单文件小改，与 SpacePage 优化风格一致）。当前并行度 2/3（5.6 + coll-click）。
- 2026-08-16 第 16 轮（双 Agent 通知触发）：5.6 验收通过 → 合并（自主验收：范围 8 文件干净、build 通过、类型与后端 inbox.rs camelCase 对齐、InboxItemDetail 预留 5.5 敏感识别扩展点、侧边栏角标 30s 轮询、「处理」按钮占位待 5.7）；coll-click 验收通过 → 合并（自主验收：范围 1 文件干净、build 通过、CollectionPage 整行 onClick + 操作列 stopPropagation）。main@5813f25 验证全绿（356 测试 + build）。worktree 全部清理，分支已删。**依赖已解锁：5.7 + 5.8 可派出。**
- 2026-08-16 第 17 轮（自动派出）：5.7（处理对话框，复用 M3-3.6 ReferenceManagedDialog）+ 5.8（合并通知 + 角标，5s 轮询 inbox_stats）。当前并行度 2/3。
- 2026-08-16 第 18 轮（Agent 通知触发）：5.8 验收通过 → 合并（自主验收：范围 4 文件干净、build 通过、useInboxStats 5s 轮询 + prevPending useRef 记录、InboxNotification 5s 合并窗口 + 启动不弹、mock __triggerInboxEvent 手动触发增量）。main@d20bfa6 验证全绿。worktree 已清理，分支已删。仍在跑：5.7。
- 2026-08-16 第 19 轮（Agent 通知触发）：5.7 验收通过 → 合并（自主验收：范围 5 文件干净、build 通过、InboxAssignDialog 622 行新建、external/managed 两阶段流程、复用 M3-3.6 ReferenceManagedDialog、INBOX_STALE 引导 dismiss_stale）。mock.ts 撞 5.8 的 __triggerInboxEvent 暴露块（保留双方 + 手工合并 mockInboxApi 结构：5.8 方法 + 5.7 方法并入同一对象 + window 暴露块移到文件末尾）。main@ac0086e 验证全绿（356 测试 + build）。worktree 已清理，分支已删。**M5 全部 8 任务收口。**
