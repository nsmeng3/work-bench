# Orchestration Protocol

## Preflight

1. Resolve the repository root, default branch, active branch, and all linked worktrees.
2. Read the selected board and task packages. Treat the board as the source of task state and each task package as the source of scope and acceptance rules.
3. Reconcile board entries with actual branches, worktrees, commits, and active agents before dispatching anything.
4. Detect stale entries instead of creating duplicates. Mark missing worktrees or vanished branches as `blocked` until reconciled.
5. Determine the concurrency limit from project configuration. Default to 3.
6. Identify shared collision surfaces such as migrations, lockfiles, generated registries, ports, databases, and central module registration files. Serialize or allocate them explicitly.

## Development cycle

Run these phases in order.

### 1. Reconcile active tasks

For every `in_progress` task:

1. Check its agent status, worktree, branch, commits, tracked changes, and untracked files.
2. If the agent is still running, leave it active and continue to another task.
3. If the agent reports completion, or the worktree contains a candidate commit, enter verification.
4. If the agent failed without making useful progress, attach the error evidence and apply the retry policy.

### 2. Verify independently

Run verification from the task worktree, not the orchestrator checkout.

1. Compare the task branch and all uncommitted changes with the task's base ref.
2. Run `scripts/check_scope.py` with every allowed path pattern from the task contract.
3. Treat any changed path outside the allowlist as a scope violation. Do not continue acceptance testing until the user resolves it or the implementation agent removes it.
4. Run the task's acceptance commands exactly as written unless the repository instructions require a stricter superset.
5. Run repository-required impact or change-detection checks before accepting a commit.
6. Move the task to `review` only when scope, acceptance, and repository checks all pass.
7. On failure, send the same task back to an implementation agent with the full failing command, exit status, relevant logs, current diff summary, and instruction to fix only the failure without expanding scope.

**Agent continuation over orchestrator fixes (MUST).** When verification, inspection, or post-merge checks reveal a functional problem, the orchestrator MUST send the work back to the original implementation agent (SendMessage to continue its session, or a fresh agent on the same worktree if the original is unavailable) rather than fixing the code itself. Rationale: (a) loop-mode's core discipline is "agents implement, orchestrator coordinates" — even mechanical-looking fixes often embed business-logic judgment; (b) the original agent has full context of its intent and can fix faster and safer; (c) orchestrator-written code blurs the approval boundary and makes evidence trails harder to audit. The ONLY exceptions where the orchestrator may edit implementation code directly are: (1) purely mechanical merge-conflict resolution in import/registration areas where both sides append to the same block and keeping both is trivially correct; (2) updating task packages, boards, or orchestration-rule documents. Anything beyond that — test failures, scope violations, structural conflicts, business-logic bugs — goes back to the agent.

### 3. Fill available capacity

While active task count is below the configured limit:

1. Select the first `pending` task whose dependencies are `done` or otherwise satisfied by the project policy.
2. Confirm that its task contract includes an objective, base ref, allowed paths, forbidden paths, acceptance commands, and approval policy.
3. Create an isolated worktree through the native agent isolation mechanism when available. If a project overlay defines persistent worktree paths or branch names, follow that overlay.
4. Record the real worktree path, branch, base commit, agent/session identifier, attempt count, and dispatch time on the board.
5. Instruct the implementation agent to read repository instructions and the task package, stay within scope, run acceptance commands, follow repository pre-commit rules, commit its changes, and return a concise evidence summary.

**Code-intelligence indexing (MUST).** When the repository uses a code-intelligence tool that builds a per-directory index (e.g. GitNexus), the implementation agent MUST build or refresh the index **inside its own worktree as the first step of the task**, before reading or editing code. The orchestrator MUST NOT pre-build the index on the agent's behalf. Rationale: (a) the agent will modify code, so any pre-built index is already stale by the time the agent needs it; (b) the index lifecycle is bound to the agent's task — the agent is the only one that knows when a rebuild is needed; (c) an orchestrator-side build cannot guarantee the index path/naming matches how the agent's tool calls resolve the repo. The orchestrator's dispatch prompt MUST include an explicit instruction like: "First step: run `<index-command>` inside your worktree at `<path>`; rebuild whenever you detect staleness."

Do not dispatch dependent tasks on top of an unapproved branch unless the task contract explicitly authorizes stacked work.

### 4. Merge approved work

For every `approved` task:

1. Reconfirm the branch, worktree cleanliness, approval identity, and verification evidence.
2. Refresh the target branch and detect merge conflicts without discarding changes.
3. Merge using the repository policy; default to a non-fast-forward merge for an auditable task boundary.
4. Run required post-merge smoke checks.
5. Mark the task `done` only after the merge and smoke checks pass.
6. Remove the worktree and task branch only when clean and safe. Preserve them and report evidence on failure.

Stop and ask the user to resolve semantic conflicts. Resolve only mechanical conflicts when the task contract or project overlay explicitly permits it.

### 5. Persist and report

Update the board atomically with status changes, evidence, attempts, and timestamps. Append a compact cycle log. Report results and the next wake-up condition.

## Gate cycle

Use gate mode only for reported defects and regression verification.

1. Register each defect with an identifier, symptom, reproduction steps, report time, and `open` status.
2. Diagnose read-only and assess impact before assigning a fix.
3. Create or reuse one isolated fix worktree. Give the implementation agent the root cause, allowed paths, forbidden paths, and acceptance commands.
4. Independently re-run the focused reproduction, task tests, and configured regression suite.
5. Move the fix through `review` and the configured approval gate. Do not advance unrelated feature tasks.
6. Record the fix commit, root cause, verification evidence, and user retest result.
7. Stop gate mode only when the user explicitly confirms that the gate passed or all configured exit conditions are met.

## Failure policy

- Retry only when new evidence or a concrete correction is available.
- Default to three verification failures per task.
- Move to `blocked` when the same blocker repeats at the retry limit, dependencies are unavailable, scope is ambiguous, or authorization is missing.
- Never convert a timeout, token limit, or missing tool into a successful result.
- If optional code-intelligence tooling is unavailable, follow the repository policy: either use its documented fallback or pause. Never silently skip a mandatory check.

