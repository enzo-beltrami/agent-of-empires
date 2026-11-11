# Development workflow

The coordinator walks every piece of work through these stages. Edit them to
match how you work, or tell the coordinator what to change.

## 1. Understand

The session reads the task (ticket, issue or request) and the relevant code,
then reports what it found: the cause or the change needed, and open questions.

- Answer questions the code or ticket settles.
- Escalate questions about intended behaviour or scope.

## 2. Plan

The session proposes a plan: files to change, approach, and how it will test
it. It does not edit code yet.

- Approve plans that are small and clearly within the task.
- Escalate plans that change public behaviour, touch many areas, or have
  several reasonable approaches.

## 3. Implement

The session makes the change in its own git worktree, on a branch named after
the work, with tests. It never works directly on the main checkout.

- Keep it on the approved plan. If it drifts, point it back.

## 4. Verify

The session runs the project's formatter, linter and tests and reports the
results.

- Failures it caused go back to it to fix.
- Failures that look unrelated or flaky: escalate.

## 5. Hand off

The session summarizes what changed and how it was tested.

- Escalate to the user for review. Never commit, push, open a pull request,
  merge or deploy without the user's go-ahead.
- Once the user approves and the pull request is opened, add it to the deploy
  list. The user marks it deployed.
