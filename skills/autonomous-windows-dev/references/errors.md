# Errors and safety

Fix the failure. Do not paper over it in a README.

## Check before you call it done

- Docs must match the code. If a command exists, the README describes it. If the README says "no write path", the code must not have one.
- Both branches of a conditional must do different things. Delete a branch that calls the same function as the other.
- A failed safety transition ends in `Blocked` (or the crate's equivalent), not a silent return that leaves the previous state looking valid.
- Flags that are documented as gates must be honored. Do not force `operator_confirmed = true` inside the workflow after the caller set it false.
- Dry-run stops before any programmer or vendor process is spawned. Dry-run must not report `Verified`, `ReadbackVerified`, or success-after-write.
- CLI and GUI use the same gate. A checkbox labeled dry-run cannot call the real write.
- Confirmation phrases are exact string matches. Do not add a `--yes` bypass.
- Build on a normal filesystem if the project directory cannot execute Cargo build scripts (`CARGO_TARGET_DIR` outside a noexec mount). A permission error is a failed build, not a pass.
- This sandbox is Linux. Windows-only APIs will not run here. Say so, and still compile the portable code. Do not tell the user the sandbox rustc is "too old" unless `rustc --version` shows that.
- `--locked` fails when `Cargo.lock` does not match `Cargo.toml`. Regenerate the lock in the same change; do not document a fake successful build.

## Redundancy

- Delete unused helpers that claim to be safe but skip checks.
- Do not copy a state machine into a third place. Update the Rust module and the Python mirror together when the gate changes.
- Comments that contradict the function (for example "does not implement writes" on a module that documents a write) get corrected in the same change.
