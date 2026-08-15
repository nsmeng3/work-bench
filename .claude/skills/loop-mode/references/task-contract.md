# Task Contract

Require a task contract before dispatching implementation. Markdown task packages are the default format.

## Required information

- Task identifier and title.
- Objective and observable completion criteria.
- Requirement, design, issue, or decision sources.
- Dependencies and base ref.
- Branch or worktree naming policy when the project controls it.
- Allowed path patterns.
- Explicitly forbidden paths or actions.
- Acceptance commands with working directories.
- Human decisions that must pause the task.
- Approval and merge policy.

Do not dispatch a task with missing scope or acceptance commands. Ask the user to complete the contract instead of guessing.

## Markdown template

````markdown
# Task <id> · <title>

## Objective

<Outcome and observable behavior>

## Sources

- <Requirement, design, issue, or decision>

## Dependencies and base

- Dependencies: <task ids or none>
- Base ref: <default branch or commit>

## Allowed changes

- `src/example/**`
- `tests/example/**`

## Forbidden changes

- `docs/contracts/**`
- Dependency or lockfile changes without approval

## Requirements

1. <Requirement>
2. <Requirement>

## Acceptance commands

```bash
<headless deterministic command>
```

## Human decisions

- <Actions that require approval>

## Merge policy

- Require user approval after independent verification.
````

## Existing-project compatibility

When using the current workbench layout:

- Read task state, dependencies, branches, and worktree paths from `tasks/*-board.md`.
- Read objectives, allowed changes, forbidden changes, requirements, and acceptance commands from `tasks/<task-id>.md`.
- Read development overrides from `tasks/loop-orchestrator.md`.
- Read gate overrides from `tasks/gate-*-orchestrator.md` and defect state from `tasks/*-gate-log.md`.
- Preserve `pending → in_progress → review → approved → done`; never infer `approved` from an agent report.

Project overlays may add stricter requirements. They must not remove independent verification, worktree isolation, scope enforcement, or the human merge gate.
