# M4 处置三档 · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> M3 起用户已授权编排者自主合并（见记忆 `feedback_m3_auto_approve`），review → done 由编排者基于独立验收证据完成。
> 并行上限：3。基线：main @ M3 门禁已通过（`066ad07`，176 后端测试 + 前端 build 全绿）。

## 里程碑目标

**M4 处置三档**（计划 W8-W9，预估 26h + backlog 扩展）：
- 归档 / 回收站删除 / 销毁三个动作完整可用
- 销毁需输入全名二次确认
- 审计可查（销毁后引用行已删，审计仍独立存活）
- 含 M3 转入的 2 条 backlog 扩展

**M4 门禁**：对同一份资源分别走归档 → 恢复 → 删除 →（回收站还原）→ 销毁，每步文件与数据状态符合详细设计 §6.7。

## 任务看板

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4.1 | 处置能力声明（`caps_json` + `disp_get_capabilities`） | feat/m4-4.1-disp-caps | /Users/differentw/data/00_Admin/workbench-trees/m4-4.1 | 后端 | M3 | done | 已合并 main（bf1d859，--no-ff），worktree 已清理，分支已删 |
| 4.2 | `disp_archive` / `disp_unarchive` | feat/m4-4.2-archive | /Users/differentw/data/00_Admin/workbench-trees/m4-4.2 | 后端 | 4.1 | done | 已合并 main（abae2d3，--no-ff；lib.rs 冲突保留双方），worktree 已清理，分支已删 |
| 4.3 | `disp_preview`（目录递归统计、可取消） | feat/m4-4.3-preview | /Users/differentw/data/00_Admin/workbench-trees/m4-4.3 | 后端 | 4.1 | done | 已合并 main（b211392，--no-ff；lib.rs + disposition.rs 双冲突手工修复），worktree 已清理，分支已删 |
| 4.4 | `disp_soft_delete`（trash crate 接入） | feat/m4-4.4-soft-delete | /Users/differentw/data/00_Admin/workbench-trees/m4-4.4 | 后端 | 4.3 | done | 已合并 main（6f6007d；disposition.rs 三方合并严重错位，手工按正确顺序拼接：destroy→map_destroy_fs_err→disp_destroy→map_trash_err→soft_delete→disp_soft_delete），worktree 已清理，分支已删 |
| 4.5 | `disp_destroy`（输入全名校验）+ 审计写入 + 引用删除 | feat/m4-4.5-destroy | /Users/differentw/data/00_Admin/workbench-trees/m4-4.5 | 后端 | 4.3 | done | 已合并 main（1008b6d，--no-ff），worktree 已清理，分支已删 |
| 4.6 | `disp_audit_list` + 审计查看界面 | feat/m4-4.6-audit-ui | /Users/differentw/data/00_Admin/workbench-trees/m4-4.6 | 全栈 | 4.5 | done | 已合并 main（7e294e8，disposition.rs 冲突保留双方 + 测试函数补全），worktree 已清理，分支已删 |
| 4.7 | 前端：三档操作按钮 + 确认对话框（销毁输入全名） | feat/m4-4.7-disp-ui | /Users/differentw/data/00_Admin/workbench-trees/m4-4.7 | 前端 | 4.5 | done | 已合并 main（7169000，前端 api 层 disposition.ts/mock.ts/types.ts 冲突手工合并），worktree 已清理，分支已删 |
| 4.8 | 【backlog】`settings_change_root_dir`（修改根目录，future_only 优先） | feat/m4-4.8-change-root | /Users/differentw/data/00_Admin/workbench-trees/m4-4.8 | 后端 | M3 | done | 已合并 main（a55343d，--no-ff；lib.rs 冲突保留双方），worktree 已清理，分支已删 |
| 4.9 | 【backlog】导入撤销（`ref_undo_import` 契约 + 撤销栈） | feat/m4-4.9-undo-import | /Users/differentw/data/00_Admin/workbench-trees/m4-4.9 | 全栈 | 4.5 | done | 已合并 main（cd2e811，--no-ff），worktree 已清理，分支已删；**越界已接受**：Agent 新增 0003 migration 扩展 disposition_audit.action CHECK（任务包契约要求 action='undo_import' 与禁改 migrations 自相矛盾，按契约落地合理） |

## 任务包文件

| # | 文件 | 状态 |
| --- | --- | --- |
| 4.1 | `tasks/m4-4.1.md` | 已起草 |
| 4.2 | `tasks/m4-4.2.md` | 已起草 |
| 4.3 | `tasks/m4-4.3.md` | 已起草 |
| 4.4 | `tasks/m4-4.4.md` | 已起草 |
| 4.5 | `tasks/m4-4.5.md` | 已起草 |
| 4.6 | `tasks/m4-4.6.md` | 已起草 |
| 4.7 | `tasks/m4-4.7.md` | 已起草 |
| 4.8 | `tasks/m4-4.8.md` | 已起草 |
| 4.9 | `tasks/m4-4.9.md` | **契约已冻结**（2026-08-16 用户批准全推荐）；依赖 4.5 完成后启动 |

## 依赖拓扑

```
M3 (done)
 ├─→ 4.1 (后端·能力声明)
 │    ├─→ 4.2 (后端·归档/恢复)
 │    └─→ 4.3 (后端·preview)
 │         ├─→ 4.4 (后端·soft_delete)
 │         └─→ 4.5 (后端·destroy + 审计)
 │              ├─→ 4.6 (全栈·审计查询+UI)
 │              └─→ 4.7 (前端·三档按钮+确认框)
 ├─→ 4.8 (后端·修改根目录) [backlog，与 4.1-4.7 并行]
 └─→ 4.9 (全栈·撤销) [backlog，依赖 4.5 的审计/删除基础设施]
```

**并行提示**：
- 第一波可并行：4.1 + 4.8（两条独立后端线；4.8 只依赖 M3）
- 4.1 入库后可并行：4.2 + 4.3
- 4.3 入库后可并行：4.4 + 4.5
- 4.5 入库后可并行：4.6 + 4.7 + 4.9
- 4.9 因涉及契约新增，需先与用户冻结契约草案

## 共享资源号段分配（防撞车）

| 资源 | 分配规则 |
| --- | --- |
| sqlx migration | M4 默认**不需新表**：`disposition_audit` 表 M1 `0001_init.sql:143` 已建好；`resource_reference.disposition` 字段与索引同文件已建。若某任务确需 schema 变更，先向用户申请并登记号段 |
| `pnpm-lock.yaml` | 4.4 需要新增 Rust 依赖 `trash` crate；先看板备注登记，编排者统一在 main 安装后各分支 rebase |
| 接口契约（详细设计 §2.6/§2.8/§4.4） | `disp_*` 5 命令 + `settings_change_root_dir` 契约已冻结；**4.9 撤销是新契约**，须先与用户确认草案后才能冻结并实现 |
| vite dev 端口 | 验收时一次只跑一个实例，不允许并行起 dev server |
| 前端 UI 库 | **Ant Design v5**（M2 决策延续） |
| 处置互斥 | 详细设计 §5：「多步写操作（托管落地、根目录迁移、销毁）通过互斥锁串行化」。4.5 destroy 与 4.8 根目录迁移共用此互斥；可复用 M3-3.2 已交付的 `MANAGED_WRITE_LOCK` 模式 |

## M3 门禁转入 backlog

详见 `tasks/m3-gate-log.md`「门禁期间建议」小节：

1. **修改根目录** → 4.8
2. **导入撤销** → 4.9

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-16 初始化：M3 门禁通过，M4 看板建立（7 主线 + 2 backlog 扩展 = 9 任务）。任务包文件待起草，等用户确认范围后逐条落 `tasks/m4-4.*.md` 并启动第一波并行（4.1 + 4.8）。
- 2026-08-16 任务包起草完成：9 份 `tasks/m4-4.*.md` 落地。4.1-4.8 契约锚点齐备可立即启动；**4.9 撤销含 5 个决策点（撤销窗口 / originalSource 存储方案 / 审计 action / 按钮位置 / move 冲突策略），需用户批准后冻结契约再启动**。待用户决策后启动第一波并行（4.1 + 4.8）。
- 2026-08-16 第 1 轮（用户启动 M4 第一波）：4.1 + 4.8 已派出，worktree 已建，Agent 首步自建索引。当前并行度 2/3。4.9 决策点待用户回复。
- 2026-08-16 第 2 轮（用户批准 4.9 契约）：全推荐方案（24h 窗口 / 扩 locator_json.originalSource / 入审计 action='undo_import' / 引用列表行操作按钮 / move 冲突报错 FS_TARGET_EXISTS）。契约已固化到 `tasks/m4-4.9.md`；依赖 4.5 审计基础设施完成后启动。
- 2026-08-16 第 3 轮（Agent 通知触发）：4.1 验收通过 → 合并（自主验收：范围 2 文件干净、197 测试全过含 23 个 disposition 新测试、disp_get_capabilities 已注册）。main@bf1d859 验证全绿。**依赖已解锁：4.2 + 4.3 可派出。** 仍在跑：4.8。
- 2026-08-16 第 4 轮（自动派出）：4.2（archive/unarchive）+ 4.3（preview 可取消）。当前并行度 3/3（4.2 / 4.3 / 4.8）。
- 2026-08-16 第 5 轮（Agent 通知触发）：4.8 验收通过 → 合并（自主验收：范围 3 文件干净、192 测试全过含 12 个 change_root_dir 新测试、MANAGED_WRITE_LOCK 已 pub(crate) 共享、审计 action='migrate' 按任务包约定仅记日志不写表）。lib.rs 撞 generate_handler! 注册区（与 4.1 的 disp_get_capabilities 同改），保留双方解决。main@a55343d 验证全绿（213 测试）。仍在跑：4.2 / 4.3。
- 2026-08-16 第 6 轮（Agent 通知触发）：4.2 验收通过 → 合并（自主验收：范围 2 文件干净、207 测试全过含 9 个 4.2 新测试、write_audit pub(crate) 辅助已抽出供 4.4/4.5 复用、disp_archive/disp_unarchive 已注册）。lib.rs 撞注册区（与 4.8 的 settings_change_root_dir 同改），保留双方解决。main@abae2d3 验证全绿（223 测试）。仍在跑：4.3。
- 2026-08-16 第 7 轮（Agent 通知触发）：4.3 验收通过 → 合并（自主验收：范围 2 文件干净、208 测试全过含 9 个 preview 新测试、disp_preview/disp_preview_cancel 已注册、PREVIEW_REGISTRY 进程级取消注册表在场）。lib.rs + disposition.rs 双冲突：lib.rs 保留双方，disposition.rs 因 git 三方合并把 4.2 的 transition_disposition 函数体错位粘贴到 preview 函数标题下，手工修复（删除错位 + 恢复正确 preview 函数体）。main@b211392 验证全绿（234 测试）。**依赖已解锁：4.4 + 4.5 可派出。**
- 2026-08-16 第 8 轮（自动派出）：4.4（soft_delete + trash crate 新依赖登记）+ 4.5（destroy + 审计 + 引用物理删除）。当前并行度 2/3。
- 2026-08-16 第 9 轮（Agent 失败）：4.4 / 4.5 Agent 均因 API 配额超限（403 usage limit）失败，未产出提交。worktree 内有中间产物：4.4 有 280 行未提交改动（trash crate 接入 + 185 行 disposition.rs），4.5 有 510 行未提交改动。用户决策：**等配额恢复后续派新 Agent 在现有 worktree 上继续**（保留中间产物）。两任务状态转 blocked。
- 2026-08-16 第 10 轮（配额恢复续派）：用户确认配额恢复，续派 4.4 / 4.5 新 Agent 接手中间产物。Agent prompt 要求：先读懂现有改动 → 对照任务包列缺口 → 补全 → 跑验收 → 提交。当前并行度 2/3。
- 2026-08-16 第 11 轮（Agent 通知触发）：4.5 验收通过 → 合并（自主验收：范围 2 文件干净、243 测试全过含 9 个 destroy 新测试、disp_destroy 已注册、MANAGED_WRITE_LOCK 复用在场、出参二选一固化为 `{ deletedRefId }`）。main@1008b6d 验证全绿。worktree 已清理，分支已删。**依赖已解锁：4.6 + 4.7 + 4.9 可派出。** 仍在跑：4.4。
- 2026-08-16 第 12 轮（自动派出）：4.6（audit_list + 审计 UI）+ 4.7（三档按钮 + 确认框）。4.9 因需复用 4.6 的审计 UI 与 4.4 的 soft_delete 基础设施，暂缓与 4.4 齐头，等 4.4 完成后再派。当前并行度 3/3（4.4 / 4.6 / 4.7）。
- 2026-08-16 第 13 轮（用户授权完全自主推进）：用户睡前授权「门禁暂时去掉，所有验收由你完成，确认没问题就可以进行下一阶段任务，如遇到问题自行判断即可」。授权已存记忆 `feedback_full_autonomy_2026_08_16.md`，覆盖此前 M3 的门禁部分。后续不再建 m*-gate-log.md，验收合并到下一阶段全自主。
- 2026-08-16 第 14 轮（Agent 通知触发 + 手工修复）：4.4 验收通过 → 合并。disposition.rs 三方合并严重错位（destroy 函数体内嵌 soft_delete 开头 / soft_delete 函数体内嵌 destroy 代码 / disp_destroy 缺闭合 / 测试函数截断），merge 与 rebase 均无法机械处理，改用「checkout --ours 取 main 含 4.5 基线 + 手工从 4.4 commit 提取 soft_delete+map_trash_err+测试 插入正确位置」策略。main@6f6007d 验证全绿（247 测试 + 2 ignored macOS 平台相关）。worktree 已清理，分支已删。仍在跑：4.6 / 4.7。
- 2026-08-16 第 15 轮（定时巡检触发）：4.6 / 4.7 验收通过 → 按顺序合并。4.6 先合（含后端 disposition.rs + 前端 api），disposition.rs 与 lib.rs 保留双方冲突；4.7 后合（纯前端），api 层 disposition.ts/mock.ts/types.ts 三方冲突手工合并（disposition.ts 取 4.7 全量 + 追加 4.6 的 dispAuditList；mock.ts 合并两个 mockDispositionApi 定义为一个；types.ts 补 DispAuditListInput 缺失的 `}`）。main@7169000 验证全绿（257 测试 + build）。worktree 全部清理，分支已删。**M4 主线 7 任务全部收口，只剩 4.9 撤销可派出。**
- 2026-08-16 第 16 轮（自动派出）：4.9（导入撤销，契约已冻结全推荐方案）。M4 最后一任务，当前并行度 1/3。
- 2026-08-16 第 17 轮（Agent 通知触发）：4.9 验收通过 → 合并（自主验收：范围 10 文件干净、273 测试全过含 16 个 undo 新测试、ref_undo_import 已注册、UNDO_WINDOW_SECS=24h 固化、MANAGED_WRITE_LOCK 互斥在场、create_managed 已追加 originalSource/managedAction 字段、前端 UndoImportDialog 在场）。**越界决策**：Agent 新增 `0003_extend_audit_action_undo_import.sql`（任务包禁改 migrations 与契约要求 action='undo_import' 矛盾；disposition_audit 无外键重建安全；符合详细设计 §3.3 只增不改迁移策略；测试全绿）。决策依据已记录，等用户晨起复核。main@cd2e811 验证全绿。worktree 已清理，分支已删。**M4 全部 9 任务收口。**
