# M2 核心三层 CRUD · 并行任务看板

> 编排者（/loop 主会话）每轮读写本文件。状态机：`pending → in_progress → review → done`。
> `approved` 只能由用户手写（验收门禁），编排者只许合并 approved 的分支。
> 并行上限：3。基线：main @ M1 门禁已通过。

| # | 任务 | 分支 | worktree | 类型 | 依赖 | 状态 | 备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2.1 | 空间服务 5 命令 + 预置工作/生活空间 | feat/m2-2.1-space-svc | ../workbench-trees/m2-2.1 | 后端 | M1 | done | 已合并 main（--no-ff），worktree 已清理，分支已删 |
| 2.2 | 资源集服务 + 聚合详情查询 | feat/m2-2.2-coll-svc | ../workbench-trees/m2-2.2 | 后端 | 2.1 | done | 已合并 main（--no-ff），worktree 已清理，分支已删 |
| 2.2b | 补实现 collection_list 命令（契约补充落地） | feat/m2-2.2b-collection-list | ../workbench-trees/m2-2.2b | 后端 | 2.2 | done | 已合并 main（lib.rs 注册冲突已解决，保留双方） |
| 2.3 | 引用服务 4 命令 | feat/m2-2.3-ref-svc | ../workbench-trees/m2-2.3 | 后端 | 2.2 | done | 已合并 main；合并后发现 ref_list 测试顺序假设缺陷，已在 main 修复（顺序无关断言），73 测试三连跑稳定 |
| 2.4 | 标签子系统 | feat/m2-2.4-tag-svc | ../workbench-trees/m2-2.4 | 后端 | 2.3 | done | 已合并 main |
| 2.5 | 筛选服务 query_refs/query_facets | feat/m2-2.5-query-svc | ../workbench-trees/m2-2.5 | 后端 | 2.3 | done | 已合并 main（lib.rs 冲突保留双方） |
| 2.6 | 健康检查/打开/访达定位 | feat/m2-2.6-health-svc | ../workbench-trees/m2-2.6 | 后端 | 2.3 | done | 已合并 main（lib.rs 冲突保留三方） |
| 2.7 | 空间列表 + 创建/编辑对话框 | feat/m2-2.7-space-ui | ../workbench-trees/m2-2.7 | 前端 | M1 | done | 已合并 main（--no-ff），worktree 已清理，分支已删 |
| 2.8 | 资源集列表 + 创建/编辑 | feat/m2-2.8-coll-ui | ../workbench-trees/m2-2.8 | 前端 | 2.7 | done | 已合并 main |
| 2.9 | 资源集详情页（分组展示） | feat/m2-2.9-coll-detail-ui | ../workbench-trees/m2-2.9 | 前端 | 2.8 | done | 已合并 main |
| 2.10 | 仅关联创建引用流程 | feat/m2-2.10-ref-create-ui | ../workbench-trees/m2-2.10 | 前端 | 2.9 | done | 已合并 main（reference.ts 冲突保留 create+update 双方）；遗留 plugin-dialog Rust 注册见 2.10b |
| 2.10b | plugin-dialog Rust 侧注册（启用系统文件选择器） | feat/m2-2.10b-dialog-plugin | ../workbench-trees/m2-2.10b | 后端 | 2.10 | done | 已合并 main，worktree 已清理 |
| 2.11 | 引用编辑/标签/生命周期控件 | feat/m2-2.11-ref-edit-ui | ../workbench-trees/m2-2.11 | 前端 | 2.10 | done | 已合并 main（详情页+mock.ts 冲突保留双方，顺手修错误码笔误） |
| 2.12 | 筛选侧边栏 + 列表页 | feat/m2-2.12-filter-ui | ../workbench-trees/m2-2.12 | 前端 | 2.9 | done | 已合并 main（types/mock/index 三处 import 冲突保留双方） |

## 共享资源号段分配（防撞车）

| 资源 | 分配规则 |
| --- | --- |
| sqlx migration | 2.1 → `0002_*`，2.2 → `0003_*`，2.3 → `0004_*`，2.4 → `0005_*`，其余后端任务不许新建 migration |
| `pnpm-lock.yaml` | 任何任务需要新依赖时，在看板备注登记，由编排者统一在 main 安装后各分支 rebase |
| 接口契约（详细设计 §2） | 冻结，任何任务不得修改；要改契约向用户申请 |
| vite dev 端口 | 验收时一次只跑一个实例，不允许并行起 dev server |
| 前端 UI 库 | **Ant Design v5**（用户 2026-08-15 决策）。任务 2.8~2.12 派出时统一附带：组件用 antd，主题定制走 ConfigProvider token，不写覆盖 antd 类名的 CSS |

## 本轮动作日志

（编排者每轮在此追加一行：时间 + 动作摘要）

- 2026-08-15 第 1 轮：无巡检对象；派出 2.1（后端，含 sqlx 接入+错误模型+0001 全表）与 2.7（前端，mock 模式）；基线 main@6f77413 前后端构建均通过；待决策：无。
- 2026-08-15 第 2 轮（Agent 通知触发）：2.1 验收通过（18 测试，范围干净）→ review；2.7 验收通过（pnpm build，范围干净）→ review；派出 2.2（基线合并了未入库的 2.1 分支）。待用户批准：2.1、2.2 合并；已知小不一致：Space.createdAt 后端 Unix 秒 / 前端类型标 string，联调时修。
- 2026-08-15 第 3 轮（Agent 通知触发）：2.2 验收通过（40 测试全过，增量仅 collection.rs+lib.rs，未触碰 2.1 文件）→ review。合并顺序提示：2.2 分支含 2.1 提交，批准时先批 2.1 再批 2.2（merge 会自动去重）。2.3 等 2.2 入库后再派出（引用层不叠加在未验收基线上）。
- 2026-08-15 第 4 轮（定时触发）：无 in_progress；无 approved 可合并；流水线空转等用户验收。
- 2026-08-15 第 5 轮（用户决策触发）：UI 库定为 **Ant Design v5**。2.7 从 review 回炉 in_progress，已派 agent 改造（Modal/Form/Table/Layout.Sider 替换手写组件，顺手修 Space.createdAt 类型）。后续前端任务（2.8~2.12）统一约定 antd v5，任务包派出时附带此约定。
- 2026-08-15：为 2.1/2.2 补三线证据（①范围 ②契约 ③行为）至看板备注，此后所有进 review 的任务按此格式给证据，用户审批 = 扫范围 + 抽查契约对照。
- 2026-08-15 第 6 轮（Agent 通知触发）：2.7 antd 改造完成 → review（范围干净、独立构建通过、createdAt 类型已修为 number）。注意 bundle 991kB（antd 全量），后续可做 manualChunks 优化，不阻塞。待批准：2.1、2.2、2.7 三项。
- 2026-08-15 第 7 轮（用户批准触发）：合并 2.1→2.2 进 main（均 --no-ff，main@a5ebe4f，40 测试全过验证绿），清理两 worktree 与分支。派出 2.3（引用服务，基线为含 2.1+2.2 的 main）。待批准：2.7（antd 改造完成，建议 `VITE_MOCK_API=true pnpm dev` 人工点验）。
- 2026-08-15 第 8 轮（用户批准触发）：合并 2.7 进 main（main@1399e1b，前后端构建均绿），清理 worktree。派出 2.8（资源集 UI，antd 约定已附带；发现契约缺口：§2.4 无 collection_list 列表命令，2.8 用 mock+TODO 过渡，建议后续补后端命令）。
- 2026-08-15 第 9 轮（用户决策触发）：契约补充 `collection_list` 已合入 main（8f52ba0，§2.4 + 总览表 41→42 + 附录命令清单同步更新）。已通知 2.8 改用真实命令；派出 2.2b 补实现该命令（在 collection.rs 追加，风格对齐既有 5 命令）。当前 3 路并行：2.3 / 2.8 / 2.2b。
- 2026-08-15 第 10 轮（Agent 通知触发）：2.3 验收通过 → review（独立复核 29 测试全过、never-touches 专项断言在场、范围干净）。待批准：2.3。仍在跑：2.8、2.2b。
- 2026-08-15 第 11 轮（用户触发跑 main 界面 + 双 Agent 通知）：main 应用已用 `VITE_MOCK_API=true pnpm tauri dev` 启动供用户点验。2.8 完成 → review（真实 collection_list，范围干净，build 通过）；2.2b 完成 → review（7 个 list_* 新测试全过，仅追加未改既有）。待批准：2.3、2.2b、2.8 三项。注意合并顺序：2.2b 依赖 2.2 已在 main，独立可合；2.8 依赖 collection_list，须等 2.2b 先入库。
- 2026-08-15 第 12 轮（用户批准三项）：按 2.3→2.2b→2.8 顺序合并。2.2b 撞 lib.rs 注册冲突（与 2.3 同改注册区），已解决保留双方。合并后发现 ref_list 测试顺序假设缺陷（created_at 同秒+UUID 随机 id 致 ORDER BY 不稳），已在 main 直接修复为顺序无关断言（4f96411），73 测试三连跑稳定。清理三 worktree。放出四路并行：2.4 / 2.5 / 2.6 / 2.9。**流程教训：同改 lib.rs 注册区的后端任务天然会撞冲突，后续后端任务合并时预期逐个解决注册冲突（机械操作，保留双方即可）。**
- 2026-08-15 第 13 轮（Agent 通知触发）：2.4 验收通过 → review（独立复核 22 测试全过、范围干净）。待批准：2.4。仍在跑：2.5 / 2.6 / 2.9。
- 2026-08-15 第 14 轮（双 Agent 通知触发）：2.6 验收通过 → review（16 测试全过 + 子进程 argv 独立传参安全核查通过）；2.9 验收通过 → review（build 通过，六分组+health 三色+mock 种子覆盖齐）。待批准：2.4 / 2.6 / 2.9。仍在跑：2.5。
- 2026-08-15 第 15 轮（Agent 通知触发）：2.5 验收通过 → review（独立复核 20 测试全过、LIKE 转义在场、范围干净）。M2 四路全部收工。**待批准：2.4 / 2.5 / 2.6 / 2.9 四项** —— 建议一次全批，我按 2.4→2.5→2.6（后端逐个解 lib.rs 冲突）→2.9（独立）顺序合并，之后放 2.10/2.11/2.12 收尾 M2。
- 2026-08-15 第 18 轮（用户批准四项）：按 2.4→2.5→2.6→2.9 合并，2.5/2.6 各撞 lib.rs 冲突均保留多方解决。main@ab12638 验证全绿（128 测试 + 前后端 build）。清理四 worktree。放出 M2 收尾三路：2.10（引用创建流程，授权新依赖 @tauri-apps/plugin-dialog 已登记）/ 2.11（引用编辑控件，可能与 2.10 撞 api/reference.ts）/ 2.12（筛选页）。
- 2026-08-15 第 19 轮（Agent 通知触发）：2.11 验收通过 → review（范围干净、build 通过、ref_update 封装在场）。待批准：2.11。仍在跑：2.10 / 2.12。
- 2026-08-15 第 20 轮（Agent 通知触发）：2.12 验收通过 → review（范围干净、build 通过、query 封装在场、全量计数取舍已注释）。待批准：2.11 / 2.12。仍在跑：2.10。
- 2026-08-15 第 21 轮（Agent 通知触发）：2.10 验收通过 → review（范围干净、build 通过、ref_create_external 封装在场）。**遗留：plugin-dialog 前端已装但 Rust 侧未注册（任务包规则正确阻止了越界），当前降级手填路径；需补一个小后端任务注册插件+加 capabilities 权限才能启用系统文件选择器**。M2 三路全部收工，待批准：2.10 / 2.11 / 2.12。
- 2026-08-15 第 22 轮（用户批准三项 + 选方案1）：按 2.10→2.11→2.12 合并。2.11 撞详情页+reference.ts+mock.ts 三处（与 2.10 同改），2.12 撞 types/mock/index 三处 import 区，全部保留双方解决；顺手修 2.11 mock 错误码笔误（COMMON_INVALID_ARGUMENT→COMMON_INVALID_PARAM）。main@8e0dd48 全绿（128 测试+前后端 build）。清理三 worktree。派出 2.10b（plugin-dialog Rust 注册）。**M2 十二任务全部合入，待 2.10b 完成后跑 M2 门禁。**
- 2026-08-15 第 24 轮（Agent 通知触发）：2.10b 验收通过 → review（范围 4 文件、双构建独立复核通过、lib.rs 注册+capabilities 权限在场）。待批准：2.10b。批准后 M2 全部收口，可跑 M2 门禁（手动走通"新建工作项目"全流程）。
- 2026-08-15 第 25 轮（用户批准触发）：2.10b 合入 main（cb9e37f），worktree 清理，main 全绿（128 测试+双 build）。**M2 全部 14 项收口（12 任务 + 2.2b + 2.10b），里程碑完成。下一步：M2 门禁 —— 用户手动走通需求 §7.1 全流程。**
