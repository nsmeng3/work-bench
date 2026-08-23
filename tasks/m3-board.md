# M3 托管与落地 · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> `approved` 只能由用户手写（验收门禁），编排者只许合并 approved 的分支。
> 并行上限：3。基线：main @ M2 门禁已通过（`3aa7ad7`，128 后端测试 + 前端 build 全绿）。

## 里程碑目标

**M3 托管与落地**（计划 W6-W7，预估 28h）：
- 首次启动初始化资源根目录（建出 6 类子目录）
- 「导入并托管」两阶段确认跑通（plan → confirmed），文件落进 `Code/Documents/...` 对应目录
- 大文件复制有进度事件，前端展示进度条
- 失败补偿：落地成功但写库失败时删除已落地文件

**M3 门禁**：从收件箱外部场景完整走一遍「导入并托管」，确认文件落进对应目录且原文件按 copy/move 正确处理。

## 任务看板

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 3.1 | 落地路径计算领域逻辑（类型→子目录、冲突建议） | feat/m3-3.1-landing-path | ../workbench-trees/m3-3.1 | 后端 | M2 | done | 已合并 main（d8a178e，--no-ff），worktree 已清理，分支已删 |
| 3.2 | `ref_create_managed` 两阶段（plan / confirmed）+ 复制移动执行 | feat/m3-3.2-managed-svc | ../workbench-trees/m3-3.2 | 后端 | 3.1 | done | 已合并 main（78e2074，--no-ff），worktree 已清理，分支已删 |
| 3.3 | 大文件复制进度事件 `managed_progress{refId, bytes, total}` | feat/m3-3.3-progress-event | ../workbench-trees/m3-3.3 | 后端 | 3.2 | done | 已合并 main（afae72e，--no-ff），worktree 已清理，分支已删 |
| 3.4 | 首次启动资源根目录初始化 `settings_init_root_dir` | feat/m3-3.4-root-init | ../workbench-trees/m3-3.4 | 后端 | M2 | done | 已合并 main（58d27c9，--no-ff），worktree 已清理，分支已删 |
| 3.5 | 前端：初始化向导界面 | feat/m3-3.5-init-wizard-ui | ../workbench-trees/m3-3.5 | 前端 | 3.4 | done | 已合并 main（ce8068e，--no-ff），worktree 已清理，分支已删 |
| 3.6 | 前端：导入并托管确认框（源→目标对比、冲突提示、copy/move 选择） | feat/m3-3.6-managed-confirm-ui | ../workbench-trees/m3-3.6 | 前端 | 3.2 | done | 已合并 main（8ca0f7d，--no-ff），worktree 已清理，分支已删 |
| 3.7 | 前端：进度条与结果反馈 | feat/m3-3.7-progress-ui | ../workbench-trees/m3-3.7 | 前端 | 3.6 | done | 已合并 main（066ad07，--no-ff），worktree 已清理，分支已删 |

## 任务包文件

| # | 文件 | 状态 |
| --- | --- | --- |
| 3.1 | `tasks/m3-3.1.md` | 已起草 |
| 3.2 | `tasks/m3-3.2.md` | 已起草 |
| 3.3 | `tasks/m3-3.3.md` | 已起草 |
| 3.4 | `tasks/m3-3.4.md` | 已起草 |
| 3.5 | `tasks/m3-3.5.md` | 已起草 |
| 3.6 | `tasks/m3-3.6.md` | 已起草 |
| 3.7 | `tasks/m3-3.7.md` | 已起草 |

## 依赖拓扑

```
M2 (done)
 ├─→ 3.1 (后端·纯函数)
 │    └─→ 3.2 (后端·两阶段命令)
 │         ├─→ 3.3 (后端·进度事件)
 │         └─→ 3.6 (前端·确认框)
 │              └─→ 3.7 (前端·进度条)
 └─→ 3.4 (后端·根目录初始化)
      └─→ 3.5 (前端·初始化向导)
```

**并行提示**：
- 第一波可并行：3.1 + 3.4（两条独立后端线）
- 3.2 入库后可并行：3.3 + 3.6（后端事件 + 前端确认框）
- 3.4 入库后可派 3.5（前端向导，依赖后端命令）

## 共享资源号段分配（防撞车）

| 资源 | 分配规则 |
| --- | --- |
| sqlx migration | M3 默认**不需要新表**（托管相关字段 M2 已建齐：`references.hosting` / `locator` 等）。若某任务确需 schema 变更，先向用户申请并登记号段 |
| `pnpm-lock.yaml` | 任何任务需要新依赖时，在看板备注登记，由编排者统一在 main 安装后各分支 rebase |
| 接口契约（详细设计 §2） | 冻结。`ref_create_managed` / `settings_get_root_dir` / `settings_init_root_dir` / `managed_progress` 事件 契约已存在，任何任务不得修改；要改契约向用户申请 |
| vite dev 端口 | 验收时一次只跑一个实例，不允许并行起 dev server |
| 前端 UI 库 | **Ant Design v5**（M2 决策延续）。组件用 antd，主题定制走 ConfigProvider token，不写覆盖 antd 类名的 CSS |
| 落地执行互斥 | 详细设计 §5 要求：多步写操作（托管落地、根目录迁移、销毁）通过互斥锁串行化；3.2 实现时须考虑 |

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-16 初始化：M2 门禁通过，M3 看板建立。任务包文件待起草，等用户确认范围后逐条落 `tasks/m3-3.*.md` 并启动第一波并行（3.1 + 3.4）。
- 2026-08-16 任务包起草完成：7 份 `tasks/m3-3.*.md` 落地，契约锚点、范围清单、验收命令齐备。待用户审阅后启动第一波并行（3.1 落地路径纯函数 + 3.4 根目录初始化命令）。
- 2026-08-16 第 1 轮（用户启动 M3）：worktree `../workbench-trees/m3-3.1` / `m3-3.4` 已建（基线 main@3aa7ad7），GitNexus 索引就绪；两 Agent 后台派出。当前并行度 2/3。
- 2026-08-16 第 2 轮（Agent 通知触发）：3.1 验收通过 → review（独立复核 12 测试全过、范围 2 文件干净、含 type_to_subdir/propose_target/suggest_rename 三函数 + 全类型映射 + 冲突改名边界用例）。待批准：3.1。仍在跑：3.4。
- 2026-08-16 第 3 轮（Agent 通知触发）：3.4 验收通过 → review（独立复核 12 测试全过 + build 无警告 + 两命令已注册 generate_handler! + 复用 0001 settings 表 key=root_dir）。范围 2 文件 432 行干净。**待批准：3.1、3.4 两项**（相互独立，可同批合并；合并顺序任意，二者都会改 lib.rs 但位置不同，预期不冲突）。
- 2026-08-16 第 4 轮（用户批准两项）：按 3.1→3.4 顺序合并（均 --no-ff），lib.rs 自动合并无冲突。main@58d27c9 验证全绿（152 测试 + 前端 build）。清理两 worktree 与分支。**M3 第一波完成，依赖已解锁：3.2（依赖 3.1）+ 3.5（依赖 3.4）可派出。**
- 2026-08-16 第 5 轮（用户授权 M3 自主推进）：用户明确"M3 整个需求就不用我批准了，你检查好，没问题就继续"。授权已存记忆 `feedback_m3_auto_approve.md`。看板状态机 `review → done` 由编排者自主完成（基于独立验收证据），其它纪律（契约冻结 / 派 Agent / 范围越界上报 / 共享资源冲突上报）不变。派出 3.2（后端 ref_create_managed 两阶段）+ 3.5（前端初始化向导）。当前并行度 2/3。
- 2026-08-16 第 6 轮（Agent 通知触发）：3.5 验收通过 → 合并（自主验收：范围 7 文件全在 app/src/**、build 通过、契约 camelCase 字段与后端 settings.rs 一致）。main@ce8068e 验证全绿（152 测试 + build）。worktree 与分支已清理。注：3.5 Agent 自报 worktree 内 `npx gitnexus analyze` 因 npx 缓存缺 `tree-sitter-swift` 失败，但改动均为新增文件+纯追加修改，无上游影响，不阻塞。仍在跑：3.2。
- 2026-08-16 第 7 轮（Agent 通知触发）：3.2 验收通过 → 合并（自主验收：170 测试全过含 18 个 create_managed 新测试、build 无警告、generate_handler! 已注册 ref_create_managed、tokio::sync::Mutex 互斥在场、范围 2 文件 1243 行干净）。main@78e2074 验证全绿。worktree 清理时遇 .claude/AGENTS.md/CLAUDE.md 未跟踪治理文件副本（Agent 复制产生，仅 GitNexus 统计数字与主仓不同），--force 清理 + 分支已删。**依赖已解锁：3.3（依赖 3.2）+ 3.6（依赖 3.2）可派出。**
- 2026-08-16 第 8 轮（自动派出）：3.3（后端进度事件，接 3.2 预留的 progress_cb 注入点）+ 3.6（前端托管确认框，复用 M2-2.10 表单基础字段组件）。当前并行度 2/3。
- 2026-08-16 第 9 轮（定时巡检）：发现 worktree 路径异常 —— 上一轮 `git worktree add ../workbench-trees/...` 因主仓 cwd 在 `app/` 子目录，相对路径解析错位，把 worktree 建到了 `workbench/workbench-trees/`。已用 `git worktree move` 迁移到约定位置 `00_Admin/workbench-trees/`，Agent 工作不受影响（正在正确目录中干活，3.3 已改 reference.rs，3.6 已改前端 3 文件）。**教训：后续 worktree add 用绝对路径，避免相对路径因 cwd 漂移出错。**
- 2026-08-16 第 10 轮（Agent 通知触发）：3.6 验收通过 → 合并（自主验收：范围 6 文件全在 app/src/**、build 通过、两阶段 confirmed 字段 + 3.7 hook `makeTempRefId`/`onRefIdReady` 在场）。main@8ca0f7d 验证全绿（170 测试 + build）。worktree 已清理，分支已删。仍在跑：3.3。3.7 依赖 3.6 done，**可派出**。
- 2026-08-16 第 11 轮（自动派出）：3.7（前端进度条，接 3.6 的 onRefIdReady 回调 + 订阅 managed_progress 事件）。当前并行度 2/3（3.3 后端 + 3.7 前端）。
- 2026-08-16 第 12 轮（Agent 通知触发）：3.3 验收通过 → 合并（自主验收：范围 1 文件 reference.rs +473 行干净、176 测试全过含 6 个 progress 新测试、节流常量 PROGRESS_BYTE_STEP=1MiB/PROGRESS_TIME_STEP=200ms 固化、Emitter 发射在场、载荷 camelCase）。main@afae72e 验证全绿。worktree 已清理，分支已删。仍在跑：3.7。
- 2026-08-16 第 13 轮（Agent 通知触发）：3.7 验收通过 → 合并（自主验收：范围 2 文件干净、build 通过、useManagedProgress hook 新建 + listen/unlisten 在场 + 契约注释指向详细设计 §4.1）。main@066ad07 验证全绿（176 测试 + build）。worktree 已清理，分支已删。**M3 全部 7 任务收口。下一步：M3 门禁 —— 用户手动走通「导入并托管」全流程，确认文件落进对应类型子目录且原文件按 copy/move 正确处理。**
- 2026-08-16 第 14 轮（M3 门禁通过）：手动走通「导入并托管」全流程（外部引用 → 收件箱待处理 → 确认框对比 → 选择托管 → 落地进 Code/Documents/Projects/... 对应类型子目录，原文件按 copy/move 正确处理），M3 门禁通过。
