---
name: run-game
description: "Build and launch the current branch's normal release game when asked to run, start, or play VERA20k."
---

# Run the game

Open the ordinary menu and leave a visible player session running.

Outside contributors: run `cargo run --release --bin vera20k` from the checkout, as in
CONTRIBUTING.md. The steps below are the maintainer's workflow.

1. Resolve the checkout with `git rev-parse --show-toplevel`.
2. Check for an existing `vera20k` process (PowerShell `Get-Process vera20k` or
   Unix `pgrep -x vera20k`). Report an existing game instead of duplicating it.
3. Build this checkout:

   ```powershell
   python -m tools.cargo_run --label <unique-label> -- build --release -p vera20k --bin vera20k
   ```

   The runner waits for active builds. Read its preserved `manifest.json` for the
   executable path; do not guess `target/release` or copy a prior build.

4. Launch that preserved executable with the primary worktree as working directory, so its
   ignored `config.toml` supplies retail-data configuration. Preserve `RA2_DIR`
   when it supplies the configured data path.
5. Remove every other inherited `RA2_*` variable from the child environment;
   restore any temporarily changed caller environment immediately after launch.
6. Confirm a responsive visible window, leave it running, and report build/branch.

Do not substitute debug, quickplay, capture, developer mode, a test harness, or
automated input. Gameplay inspection, screenshots, and parity claims require their
own requested scope.
