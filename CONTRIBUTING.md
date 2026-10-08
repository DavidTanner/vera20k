# Contributing to VERA20k

Thanks for your interest. You don't need reverse-engineering experience to start, and some
useful work needs no code at all. Ask questions in your issue or on
[Discord](https://discord.gg/kmjRUn5m5F).

## Ways to help

- **Pick an issue.** A [`good first issue`](https://github.com/YuriPlanet/vera20k/labels/good%20first%20issue)
  fits an evening and says what "done" means.
- **Compare with the original.** Play the same situation in Yuri's Revenge and in VERA20k and
  report what differs, ideally with a clip of each. No code needed, and very useful.
- **Code.** Gameplay is in `src/sim/`, rendering in `src/render/`, menus and input in
  `src/app/`, `src/ui/` and `src/sidebar/`, tools in `src/bin/` and `tools/`.

For anything large, ask first.

## Set up

1. **Get the game** (see the [README](README.md#vera20k)). Any edition of Yuri's Revenge 1.001
   works; on macOS copy the folder from a Windows install. Many issues need no game at all.
2. **Install** current stable Rust from [rustup](https://rustup.rs/), and have a GPU with
   Vulkan, DirectX 12 or Metal. Debian/Ubuntu also need `libasound2-dev` and `pkg-config`.
3. **Build and run:**

   ```sh
   git clone https://github.com/YuriPlanet/vera20k.git
   cd vera20k
   cp config.toml.example config.toml   # Windows cmd: copy config.toml.example config.toml
   cargo run --release --bin vera20k
   ```

   Set `ra2_dir` in `config.toml` with forward slashes (`C:/Games/RA2`). Always play with
   `--release`; the log is in `logs/ra2.log`.
4. **Run the tests:** `cargo test -p vera20k --lib` (always `--lib`). Tests that need the
   game's INI files skip themselves and still count as passed until you run
   `cargo run --bin extract-ini [game folder]`.
5. **Never commit game files** (`.mix`, INI, art, audio, video, `.exe`) or anything in `ini/`.

You don't need the Python tools in `tools/` to contribute. The [tool index](tools/README.md)
lists them; most serve the maintainer's own workflow.

## Your first pull request

1. **Claim an issue** by commenting. One at a time; a claim with no update for 14 days is free.
2. **Branch from `main`.** One issue per PR, at most one gameplay mechanism. The issue's
   stated done-criteria bound the PR; anything you find beyond them becomes a new issue.
3. **Check it:** `cargo test -p vera20k --lib` and `cargo clippy -p vera20k --lib`. Main
   already has hundreds of Clippy warnings, so fix only new ones in code you changed. Format
   only the files you changed with `rustfmt --edition 2024 <file>`. Don't run `cargo fmt` or
   `cargo clippy --fix`; they rewrite unrelated code.
4. **Say in the PR if a feature starts or stops working.** The maintainer updates the README
   status and its translations.
5. **Open the PR against `main`.** Say what changed, how you checked it and which issue it
   closes (`Closes #123`).

Until your first PR is merged, its CI waits for the maintainer to approve each run. The one
required check fails if the change adds a `pub` or `pub(crate)` field to a struct under
`src/sim/`; keep new fields private and change them through their owner. Run it locally with
`python tools/sim_field_ratchet.py --base origin/main`.

## Project rules

[`AGENTS.md`](AGENTS.md) is the contract for the maintainer's own agents; outside contributions
follow this file and the issue. These rules matter:

1. **The original is the reference.** Cite the native function in a comment, as the code
   around it does: `/// MissionClass::Mission_Dispatch @ 0x005B3060`. Don't guess.
2. **One owner per piece of state.** Extend the existing owner; delete the old path you replace.
3. **Deterministic simulation.** Same inputs, same result on every OS and CPU. Use `SimFixed`
   in `src/sim/`, and keep random draws and same-frame effects in the original's order.
4. **AI tools are welcome.** Say in the PR which parts they wrote, and be ready to answer
   questions about the change in review. Coding agents such as Codex and Claude Code load
   `AGENTS.md` or `CLAUDE.md` automatically; their opening lines tell your agent to follow
   this file and the issue instead.

## Evidence

Say where the behavior comes from: an original function or address, the issue, or something you
saw in the original game. For gameplay changes, add a test. You can open the PR for review
without a critic review, manual play or a comparison with the original game running. The
maintainer supplies native evidence for starter issues and can run comparisons during review.
If you do build a native comparison, commit only the harness and the corpus a test reads, with
its meta sidecar. Test results, captures and logs go in the PR description.

## Review and bugs

The maintainer reviews every outside PR and aims to reply within a few days; ping the PR or
Discord if it goes quiet. Report bugs and differences with the
[issue forms](https://github.com/YuriPlanet/vera20k/issues/new/choose), including the commit,
map, OS, GPU and a clip or log. To learn the code, start with the
[architecture overview](https://yuriplanet.github.io/vera20k/).

## License

GPLv3 ([`LICENSE-GPL`](LICENSE-GPL)). Contributions are licensed the same way; there is no CLA.
