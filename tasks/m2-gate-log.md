# M2 门禁日志

> 门禁目标：手动走通需求 §7.1"新建工作项目"全流程（建空间→建资源集→关联代码库/文档/制品→按类型查看）。
> 基线：main@cb9e37f，128 后端测试 + 前端 build 全绿。
> 运行方式：`cd app && pnpm tauri dev`（真实模式，数据落 SQLite）。

## Bug 记录

| # | 现象 | 复现步骤 | 报告时间 | 状态 | 修复提交 | 根因 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 编辑对话框不回显数据 | 空间列表/资源集列表点"编辑" → 表单为空 | 2026-08-15 | fixed | bde0935 | `destroyOnHidden` 让 Modal 关闭时销毁表单实例，重开时 `setFieldsValue` 被吞。三处（空间/资源集/引用编辑）统一改 `forceRender` 让表单常驻。**流程纠偏：此修复由编排者直接改代码完成，违反"一律派 Agent"规则；规则已更新明确角色边界，后续 bug 一律派 Agent** |
| 2 | 筛选页打开无数据且报错 | 打开"筛选"菜单 → 左侧筛选维度空白，弹出错误提示 | 2026-08-15 | fixed | 7ef6393 | 契约字段名不匹配：后端 `QueryFacetsResult` 序列化输出复数名 `{types, lifecycles, confidentialities, tags}`（`app/src-tauri/src/query.rs:53-58`），前端 `QueryFacetsOutput` 期望单数名 `{type, lifecycle, confidentiality, tags}`（`app/src/api/types.ts:230-235`）。`FilterPage.tsx` 读 `facets.type/lifecycle/confidentiality` 拿到 `undefined` → `FacetGroup.values` 为 `undefined` → `values.length` 抛 TypeError → message 报错。**修法：前端对齐后端（后端已有测试固化复数名），改 types.ts / FilterPage.tsx / mock.ts 三处** |
| 3 | 筛选页报错 `Command query_refs not found` | 打开"筛选"菜单 → 弹错误提示 | 2026-08-15 | fixed | 608cc8d | Tauri v2 `generate_handler!` 宏按函数名原样暴露命令；后端命令函数用了 `_cmd` 后缀（`query_refs_cmd` / `query_facets_cmd`，`app/src-tauri/src/query.rs:392,426`），注册后暴露名就成了 `query_refs_cmd`，与前端 `invoke("query_refs")` 不匹配。其它 14 个命令都是裸名（`space_create` 等），所以只有这两个 404。**修法（用户选 A）：命令函数去 `_cmd` 后缀拿裸名；原内部业务函数改名 `query_refs_impl` / `query_facets_impl`；测试模块同步改 ~30 处** |
| 4 | 筛选页列表「所属资源集」列显示 id 而非名称 | 打开"筛选"菜单 → 引用列表"所属资源集"列显示 `mock-collection-1` 之类的原始 id | 2026-08-15 | fixed | 458bd3d | `FilterPage.tsx` 的 columns 直接渲染 `collectionId` 原始 id，未做 id→name 映射。**流程纠偏：编排者在 /loop 模式下曾直接改动该文件，违反"一律派 Agent"规则；已回滚并由 Agent 在独立 worktree 中按最小修复原则重做** |

## 门禁结论

**2026-08-16 门禁通过**（用户确认）。

- 基线：main@3aa7ad7，128 后端测试 + 前端 build 全绿。
- 走通范围：需求 §7.1「新建工作项目」全流程（建空间 → 建资源集 → 关联代码库/文档/制品 → 按类型查看）。
- 期间共登记并修复 4 条 bug（编辑回显 / facets 字段名 / query_refs 命令名 / 资源集名称显示），全部回归通过。
- 下一步：进入 M3 规划。
