---
name: loop-mode
description: Start and manage a bounded implementation loop with isolated Git worktrees, delegated implementation agents, deterministic verification, a task board, and human-gated merges. Use when the user says “启动 loop 模式”, “进入 loop 模式”, “继续 loop”, “停止 loop”, “用 loop 完成任务”, “持续执行直到验收通过”, or asks Claude Code to orchestrate parallel tasks or gate-period bug fixes through worktrees.
---

# Loop Mode

Act as the orchestrator. Keep implementation work out of the orchestrator checkout, delegate it to isolated worktrees, verify it independently, and require the configured human gate before merging.

## Load the protocol

1. Read [references/orchestration.md](references/orchestration.md) before starting or continuing a loop.
2. Read [references/task-contract.md](references/task-contract.md) before interpreting, creating, or dispatching task packages.
3. Read [references/state-machine.md](references/state-machine.md) before changing task status, scheduling another cycle, merging, or stopping the loop.

Treat repository instructions and project overlays as project-specific additions. Never let them weaken the safety and approval rules in this skill.

## Start or resume

1. Resolve the repository root with `git rev-parse --show-toplevel`. Stop if the current directory is not in a Git repository.
2. Read applicable `CLAUDE.md`, `AGENTS.md`, and project settings before taking action.
3. Discover project overlays in this order:
   - `.loop/config.md` or `.loop/config.yaml` when present.
   - `tasks/loop-orchestrator.md` for development mode.
   - `tasks/gate-*-orchestrator.md` or `tasks/gate-orchestrator.md` for gate mode.
   - `tasks/*-board.md` for task state and dependencies.
   - Task packages referenced by the selected board.
4. Inspect `git status --short --branch` and `git worktree list --porcelain`. Preserve unrelated or pre-existing changes.
5. Select the mode from the user request and project state:
   - Use `development` to advance pending implementation tasks.
   - Use `gate` to register, repair, verify, and close reported bugs without starting new feature work.
   - Continue the active mode when the user says “继续 loop”.
6. Run one orchestration cycle immediately. Do not wait for a scheduled wake-up before doing useful work.

If no board or task contract exists, do not invent a broad implementation. Ask for the task objective and acceptance criteria, then create or propose a contract using the template.

## Schedule the loop

Treat “启动 loop 模式” as authorization to run the immediate cycle and keep monitoring active background work in the current session.

- Prefer completion notifications from background agents.
- When a session-scoped scheduling tool is available and background work needs polling, create one recurring wake-up using the project cadence; default to 10 minutes when the project does not specify one.
- Record the scheduler job identifier in the board or loop status, not only in conversation memory.
- Keep at most one scheduler job per repository and mode.
- Delete the scheduler when there is no runnable or active work, the gate passes, the loop reaches a terminal state, or the user says “停止 loop”.
- Never implement a shell-level infinite loop or unbounded retry loop.

## Enforce boundaries

- Do not edit implementation files in the orchestrator checkout. Allow only task, board, log, and orchestration metadata updates there.
- Use an implementation agent with worktree isolation. If isolated delegation is unavailable, report the limitation instead of silently editing in place.
- Never trust an agent's completion claim. Re-run scope checks and acceptance commands independently.
- Never merge a task that is not in `approved` state. Only the user or an explicitly configured external approval gate may grant `approved`.
- Never force-push, delete a dirty worktree, discard pre-existing changes, or bypass repository permissions.
- Honor dependency order, concurrency limits, protected paths, repository-specific impact analysis, and pre-commit checks.
- Pause before high-risk or scope-expanding actions, including contract changes, migrations outside an assigned range, new dependencies, production access, destructive operations, or shared infrastructure changes.
- Bound retries by the project configuration; default to three failed verification cycles per task. Move repeated failures to `blocked` with evidence.

## Report each cycle

Keep the user-facing update concise and include:

- Tasks verified as passed or failed.
- Tasks dispatched or resumed.
- Items waiting for approval or a human decision.
- Current counts by status.
- The next wake-up condition, or confirmation that the loop stopped.

