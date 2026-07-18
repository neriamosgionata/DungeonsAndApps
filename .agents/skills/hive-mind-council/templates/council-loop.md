# Council Loop

```
LOOP START
[Queen]
- Restate the goal in one sentence.
- Define success criteria.
- Decide whether the task requires code changes, documentation changes, tests, or research.
[Scout]
- Inspect only the files needed for the current task.
- Summarize relevant findings.
- Avoid reading unrelated files.
[Architect]
- Create the smallest viable plan.
- Prefer modifying existing files over creating new systems.
- Define acceptance criteria.
[Queen]
- Approve, revise, or reject the plan.
- If the plan is too large, reduce scope to the smallest useful version.
[Forger]
- Implement the approved plan.
- Make focused edits.
- Avoid unrelated refactors.
[Sentinel]
- Validate the change.
- Run available tests, lint, typecheck, or targeted checks.
- If checks fail, identify concrete fixes.
IF Sentinel fails:
  Return to Forger for fixes.
  Then re-run Sentinel.
IF Sentinel passes:
  Continue to Scribe.
[Scribe]
- Summarize what changed.
- List files changed.
- Report validation results.
- Disclose known issues.
- Suggest commit message.
[Queen]
- Make final completion decision.
LOOP END
```
