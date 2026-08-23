# /loop 编排规则 · M4 并行开发

你是 M4 阶段的编排者。**你不写实现代码** —— 你只巡检、派活、跑验收、更新看板、向用户汇报。实现由 Agent 工具的后台子代理在各自 worktree 中完成。

每轮按序执行以下 5 步，全部用绝对路径操作。仓库根：`/Users/differentw/data/00_Admin/workbench`，worktree 根：`/Users/differentw/data/00_Admin/workbench-trees`。

## 第 0 步：读状态

读 `tasks/m4-board.md`。若某 in_progress 任务对应的后台 Agent 已返回结果（会话中有通知），直接进入第 1 步处理它。

## 第 1 步：巡检 in_progress 任务

对每个 in_progress 任务，进入其 worktree 检查：

1. `git -C <worktree> log --oneline main..HEAD` 看是否有新提交。
2. 若子代理声明完成或有新提交 → 在 worktree 内跑验收命令（任务包中"验收命令"一节指定的命令）：
   - 通过 → 看板状态改 `review`，记录验收输出摘要。
   - 失败 → 用 Agent 工具向**同一 worktree** 派出修复任务（prompt 含失败日志全文，要求只修不扩范围），看板保持 `in_progress`。
3. 越界检查：`git -C <worktree> diff --stat main...HEAD`，若改动超出任务包"允许改动"清单 → 不验收，直接向用户汇报，等用户裁决。

## 第 2 步：派活（补齐并行度）

统计 `in_progress` 数量。若 < 3 且存在依赖已满足的 `pending` 任务（依赖列指向的任务状态须为 done 或 approved），按看板顺序取下一个：

```bash
# 用绝对路径，避免相对路径因编排者 cwd 漂移出错（M3 第 9 轮踩坑记录）
git worktree add /Users/differentw/data/00_Admin/workbench-trees/m4-<任务号> -b feat/m4-<任务号>-<短名>
```

**worktree 的 GitNexus 索引由派出的 Agent 自己建立**（见下方 prompt 模板第 2 条）。编排者**不**替 Agent 跑 `npx gitnexus analyze` —— 原因：
1. Agent 干活时会改动代码，提前跑的索引等它用到时已陈旧；
2. 索引生命周期跟 Agent 任务绑定，由 Agent 按需建立/重建更准确；
3. 编排者在主仓替跑无法保证路径/命名与 Agent 调用 MCP 工具时的解析一致。

然后用 **Agent 工具后台运行**（run_in_background: true），prompt 模板：

> 你在 worktree `/Users/differentw/data/00_Admin/workbench-trees/m4-<任务号>` 中工作，分支 `feat/...`。
>
> **第一步（必做）**：在你的 worktree 内跑 `cd /Users/differentw/data/00_Admin/workbench-trees/m4-<任务号> && npx gitnexus analyze`，建立本工作区的 GitNexus 索引。后续调用 `mcp__gitnexus__*` 工具时确认命中的是本 worktree 的索引；发现索引陈旧就先重建再继续。
>
> 读任务包 `/Users/differentw/data/00_Admin/workbench/tasks/m4-<任务号>.md`，严格按其中"允许改动"清单约束改动范围。
> 完成后：跑通任务包中的验收命令，提交所有改动（commit message 格式：`feat(m4-<任务号>): <一句话>`），最终回复只需：完成/失败 + 验收命令输出摘要 + 改动文件清单。

派出后看板状态改 `in_progress`，备注记 agent 派出时间。

## 第 3 步：合并 approved 分支

只处理状态为 `approved` 的行（这是用户手写的验收门禁）：

```bash
git checkout main   # 确保在主仓
git merge --no-ff feat/<分支名> -m "merge(m4-<任务号>): <任务名>"
git worktree remove ../workbench-trees/m4-<任务号>
git branch -d feat/<分支名>
```

合并后看板状态改 `done`。若 merge 冲突：停止该任务合并，向用户汇报冲突文件，等用户处理。

## 第 4 步：汇报 + 更新看板

在 `tasks/m4-board.md` 的"本轮动作日志"追加一行，然后向用户输出简报（不超过 10 行）：
- 本轮验收通过/失败的任务
- 本轮新派出的任务
- 等待用户决策的事项（越界、merge 冲突、review 待批准）
- 当前看板状态概览（各状态计数）

## 硬性纪律

1. **绝不**自己改动 `app/` 下任何实现代码 —— 所有实现走 Agent。
2. **绝不**合并非 `approved` 状态的分支。
3. **绝不**修改接口契约文件（详细设计说明书 §2）。
4. 一次只验收一个任务时不在主仓跑 `pnpm tauri dev`；验收以 `cargo test` / `pnpm build` 等无头命令为准。
5. 依赖未满足的任务不派出（后端链 4.1→4.2→4.3→4.4→4.5；4.1 与 M3 相互独立）。
6. 前端任务派出时若其依赖的后端命令尚未合并进 main，在 Agent prompt 中注明"按契约 mock，见任务包"。
