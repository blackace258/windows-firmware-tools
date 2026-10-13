---
name: autonomous-windows-dev
description: Autonomous Windows source development. Use when the conversation is about writing, changing, or debugging software that runs on Windows. Triggers on feature requests, bug reports, refactors, and any request to produce working Windows-compatible code. Start implementing immediately; do not ask for permission to code.
---

# Autonomous Windows development

Turn the conversation into working software for a Windows computer. The latest user instruction wins when it conflicts with an earlier one.

Read `references/directories.md` before creating folders or a second copy of an existing tool. Read `references/errors.md` before claiming a build, test, or push succeeded.

## Rules

1. Inspect the current tree first. Extend the project that already owns the behavior. Do not add a parallel implementation, empty placeholder directory, or a second language port unless the user asked for one.
2. Start editing as soon as the task is clear. Never ask "should I proceed?" or "do you want me to write the code?".
3. Ask only when a missing fact would make the result wrong or unsafe (exact hardware identity, a secret you must not invent, two mutually exclusive behaviors). Otherwise pick a sensible default and continue.
4. Ship complete, runnable code in the project's language and Windows conventions (paths, APIs, line endings for scripts the user will run on Windows). No pseudocode, stubs, or "left as an exercise".
5. Build and test when the toolchain exists. If it does not, finish everything that does not need it and name the exact blocker. Never report a build, test, or git push you did not run.
6. Autonomy is not permission to destroy unrelated data, expose secrets, weaken a safety gate, or skip a confirmation the existing design requires.

## Done

Summarize what changed, what was verified, and any remaining blocker. Keep it short.
