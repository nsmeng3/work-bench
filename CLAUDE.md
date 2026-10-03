<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **workbench** (3747 symbols, 8542 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/workbench/context` | Codebase overview, check index freshness |
| `gitnexus://repo/workbench/clusters` | All functional areas |
| `gitnexus://repo/workbench/processes` | All execution flows |
| `gitnexus://repo/workbench/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

# 项目约定

## 桌面通知统一走悬浮窗

应用内所有「需要主动提醒用户」的桌面通知，**统一使用悬浮窗通道**，不要再引入系统级通知（tauri-plugin-notification / osascript 等）。

用法（前端）：

```ts
import { invoke } from "@tauri-apps/api/core";

await invoke("float_notify", {
  title: "通知标题",
  body: "通知正文（单行，超长自动省略）",
  action: "go-inbox", // 可选：点击卡片后向主窗口 emit 的事件名；不传则点击仅唤起主窗口
});
```

- 实现：Rust [app/src-tauri/src/float.rs](app/src-tauri/src/float.rs) + 前端 [app/src/float/FloatApp.tsx](app/src/float/FloatApp.tsx)。
- 主窗口通过 `listen(action)` 响应点击跳转（参考 App.tsx 的 `go-inbox`）。
- 高频事件先在业务侧做合并/去抖再调 `float_notify`（参考 InboxNotification 的 5s 合并窗口），避免悬浮窗刷屏。
- 应用内即时反馈仍可用 antd message/notification；悬浮窗只用于「用户可能不在看应用」的场景。
