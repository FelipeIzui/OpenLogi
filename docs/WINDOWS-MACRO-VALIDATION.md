# Windows macro editor — PR #1 validation

This document validates the first workflow-composer increment. It is **not** a
G HUB-style recorder: keyboard/mouse event recording, repeat modes, dedicated
macro library, and mouse-button assignment are outside this PR.

## GitHub Actions prerequisite

The fork currently has no workflow runs (including none on this PR). In a new
GitHub fork, Actions may require explicit activation by a repository admin:

1. Open `https://github.com/FelipeIzui/OpenLogi/actions`.
2. If GitHub displays an **I understand my workflows, go ahead and enable them**
   banner, enable Actions for this fork.
3. Open [PR #1](https://github.com/FelipeIzui/OpenLogi/pull/1) and inspect
   checks for the **current head commit**. GitHub may require rerunning or
   re-triggering checks after enabling.
4. Do **not** merge while any required check is absent, pending, or failing.
   A green PR on Linux/macOS alone does not verify the Windows injector.

The existing `.github/workflows/ci.yml` handles PR checks; avoid making a
second, duplicate workflow solely for this change.

## Local Windows prerequisites

- Windows 11 and Git.
- Rust stable via [rustup](https://rustup.rs/); verify `rustc --version`,
  `cargo --version`, and `rustup show`.
- Visual Studio C++ Build Tools for the MSVC toolchain, including the
  C++ desktop workload and Windows SDK, if not already installed.
- Quit Logitech G HUB / Logi Options+ and any installed OpenLogi agent
  before manually launching a development build, since devices/receivers
  may only be owned by one program.
- Back up your existing OpenLogi TOML configuration before running a
  development build that shares the installed app's configuration.

Open PowerShell:

```powershell
git clone https://github.com/FelipeIzui/OpenLogi.git
cd OpenLogi
git switch --track origin/feat/windows-workflow-macro-editor

rustup component add rustfmt clippy
$env:RUSTFLAGS = "-D warnings"
cargo fmt --all -- --check
cargo test -p openlogi-inject
cargo test -p openlogi-desktop
cargo clippy -p openlogi-inject -p openlogi-desktop --all-targets -- -D warnings
cargo build -p openlogi-agent -p openlogi-desktop --release
```

Check each process exit code. **No result should be marked passed just
because the command was written here.** Save the failing command and full
error output for diagnosis if any step fails.

The project-wide pre-push verification is documented in
[`AGENTS.md`](../AGENTS.md) and [`docs/DEVELOPMENT.md`](DEVELOPMENT.md).
The narrow commands above are a first-pass debug loop, **not** a substitute
for the repository's full pre-push gate and CI.

## Manual smoke test — Windows 11

Run the locally built agent and desktop GUI from the same release directory
(for example, separate PowerShell windows). The agent must be running for
the UI to show connected devices.

```powershell
.\target\release\openlogi-agent.exe
# in a second PowerShell window:
.\target\release\openlogi-desktop.exe
```

1. In **Keys**, select a programmable function key and open **Create Macro…**.
   Confirm the editor offers **Text**, **Shortcut**, and **Delay**.
2. Add **Text:** `Olá 😊`. Add **Delay:** `250` (milliseconds).
   Add **Shortcut:** `Ctrl+A`. Click **Save Workflow**.
3. Open Notepad and press the bound function key. Expect the accented text
   and emoji to appear and then to be selected, after the delay.
   **This is the hardware/runtime test — source review is not a pass.**
4. Reopen the workflow, edit the text step, and save. Repeat in Notepad;
   the new text must replace the previous value rather than append to it.
5. Add a temporary step, click **Cancel**, and reopen. Confirm the last
   persisted steps are unchanged.
6. Try an invalid shortcut and delays `0` and `60001`; the editor
   must reject these inputs without committing partial state.
7. Remove all steps. Confirm the editor keeps the draft empty rather than
   silently repopulating it from the last saved workflow.
8. Test a long string (including surrogate-pair emoji) to exercise
   `SendInput` batching.
9. Restart the app and verify the saved workflow persists.
10. Verify the remapped function key, ordinary mouse movement/clicks,
    DPI/SmartShift, and Powerplay receiver behavior were not regressed.

## Exit criteria

- Formatting, focused tests, and relevant Clippy checks pass.
- All existing mandatory GitHub CI jobs pass on the current head.
- Real Windows 11 hardware tests above pass.
- No input recursion, stuck modifiers, crashes, or unintended repetition.
- The PR is explicitly reviewed before being marked ready or merged.

Keep the PR as a draft until these criteria are evidenced.
