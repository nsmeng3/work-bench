# Loop State Machine

## Development states

| State | Meaning | Who may enter it |
| --- | --- | --- |
| `pending` | Contract exists but dependencies or capacity have not selected it | Planner or user |
| `in_progress` | An implementation agent owns an isolated worktree | Orchestrator |
| `review` | Independent scope and acceptance checks passed | Orchestrator/verifier |
| `approved` | The configured human or external gate approved the verified result | User or configured gate only |
| `done` | Approved work merged and post-merge checks passed | Orchestrator |
| `blocked` | Progress requires missing input, dependency, authorization, or recovery | Orchestrator |
| `failed` | Bounded attempts ended with a non-recoverable failure | Orchestrator |
| `cancelled` | The user cancelled the task | User |

Use `verifying` and `merging` as transient runtime states when the board supports them. Otherwise record these phases in the task evidence without changing the compatibility state sequence.

## Allowed transitions

```text
pending → in_progress
in_progress → review
in_progress → blocked | failed | cancelled
review → in_progress          verification or requested changes failed
review → approved             user or configured gate only
approved → done
approved → blocked            merge or post-merge check failed
blocked → in_progress         blocker resolved with new evidence
```

Do not transition directly from `in_progress` to `approved` or `done`. Do not treat a commit, passing self-reported test, or agent completion message as approval.

## Gate states

```text
open → diagnosing → fixing → review → fixed
                    ↘ blocked
```

Require the user retest when the gate procedure calls for manual reproduction. Keep `fixed` distinct from “gate passed.”

## Evidence per transition

Record at least:

- Timestamp.
- Actor or agent/session identifier.
- Branch, worktree, base commit, and result commit.
- Changed-file summary and scope-check result.
- Acceptance commands with exit status.
- Attempt count and failure fingerprint.
- Approval identity and time for `approved`.
- Merge commit and post-merge check for `done`.

## Scheduler lifecycle

Start one scheduler only when active work needs monitoring. Keep it while tasks are `in_progress`, `review` awaits an external signal the loop must observe, or gate mode is actively waiting for reported defects by explicit user request.

Stop and delete the scheduler when:

- No runnable or active task remains.
- Every remaining task is `done`, `failed`, `cancelled`, or requires direct user input.
- The gate passes.
- The user says “停止 loop”.
- The session can no longer safely resume its recorded repository and board.

