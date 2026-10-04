Read-only critic: selected local ground Unit ordinary Move action lines

Reviewed implementation d5b667327a69b2a2d16578e938cfc8382a0820ed against b0b6dafc63750a368e4eee0abc91a1803664a820, plus the supplied dirty production archive/comparator/documentation changes. The scope is selected local stock MTNK ordinary Move/reissue, planning off. This is the single fresh critic pass; no implementation, Git, Ghidra or process changes were made. The only written file is this ignored report.

No blocking implementation defect found in the reviewed chain. This does not certify all procedural drawing, all Unit destinations, or a native whole Scenario.

Actionable finding, ordered by severity:

- P3, documentation reproduction command: tools/procedural_drawing_oracle/action_lines.md:13 originally instructed `python tools/procedural_drawing_oracle/action_lines.py --check` from the repository root. With PYTHONPATH unset this fails immediately with `ModuleNotFoundError: No module named 'tools'`, before native validation runs. The module form used by README (`python -m tools.procedural_drawing_oracle.action_lines --check`) avoids the import-path dependency. The owner reported correcting this during the review and is retaining an exact-command rerun. This is not a runtime defect.

Evidence and implementation conclusions:

- Read AGENTS.md and .agents/skills/_shared/review.md first. Traced the live input restart sites, selected/House/option admission, NavCom/last-queue selection, source Location and target +48 coordinate owners, bridge replacement, timer expiry/load reanchor, ordinary Move class dispatch, Stop/arrival cleanup, native pixel leaves, production draw submission and dense overwrite composition.
- Inspected the original instruction packet for Foot4DC060, Tactical6D4750, Move event4C7467..4C747C and Unit741970 same-destination admission. The Move route uses raw Location, requires nonnull NavCom, reads the last queued Cell when present, and adds the initialized bridge offset only behind the native bounds/flag gate. The existing House owner implements the native current-house/campaign distinction; no new simulation authority was introduced.
- Ordinary nonqueued represented Unit Move now reaches the existing Unit setter with clear_queue1. The repeated-target early return preserves the native queue/timer state. The command's preexisting speed lookup is read-only; Drive samples its live speed separately. The two corrected old regression assumptions are backed by native execution rather than new expected values invented in Rust. The explicit queued extension remains on its existing route and is not certified here.
- CdTimer/binary_frame replaces saturating tick-age arithmetic. Native executed controls cover restart, signed wrap, sentinel, expiry and successful-load remainder reanchoring. The application load commit is the sole production PreparedLoad commit caller and reanchors after the restored frame is installed. The timer is not silently added to simulation snapshots or hashes.
- The shared solid walk and rectangle intersection remove the duplicated radar/map implementations. Direct and dense action-line paths both rasterize native logical pixels before zoom; the dense buffer is ephemeral and preserves ordered opaque overwrite. Presentation terrain queries isolate the shared Dummy and cannot mutate simulation through rendering.
- Status/brackets/pips now precede effects, bandbox, second rally and action lines, consistent with the inspected native caller ordering. The selected line uses the existing UI passthrough pipeline with alpha1 and no depth read/write. Mixed green/red primitive tests establish overwrite ordering without claiming Attack anchor correctness.

Validation independently checked during this pass:

- Original action_lines --check: PASS, no files written (PYTHONPATH supplied for the originally documented direct invocation after reproducing its import failure).
- `python -m tools.spatial_oracle.track_destination --check`: PASS, no files written.
- `python -m tools.spatial_oracle.track_speed_native --check`: PASS, no files written. Compared against the base JSON: all75 getter and116 prefix rows are unchanged;16 retained history steps are added.
- Portable Move production archive recheck: PASS. It checks the original native-bound captures and12 final replays, including5760000 identical frame pixels and385 observed boundaries. Inspected the retained active/stopped PNGs; observed source/endpoint and cleanup agree with the paired evidence.
- Independently verified all22 saved validation archive file lengths, plain SHA256s and gzip SHA256s. Actual output confirms full retail lib9671 passed/239 ignored, optimized88 passed, Python594 passed/5 optional skips, Clippy completion and field ratchet2505/2505. Existing failed attempts remain retained and explained. No Cargo was run by the critic.
- Recomputed workload means from retained samples. The reported20k overlapping stress numbers agree:979995 to63 spans;121519380 to7812 bytes; CPU build14.422 to8.266ms (final8.541), staging10.939 to0.025ms (final0.022), completion27.244 to1.538ms (final1.533). GPU timestamps remain unavailable; no GPU-duration or whole-game FPS conclusion follows.
- `git diff --check`: clean for the then-current dirty evidence updates.

Useful cleanup, not a blocking defect: src/sim/world/ground_move.rs:1-18 still describes command orders generally as reaching issue_ground_move, although this PR routes ordinary represented Unit Move directly to set_unit_destination. Updating that ownership description would make future tracing of repeated-NavCom behavior clearer. No additional owner, cache or abstraction is warranted by this review.

Coverage limits and remaining goal work:

- Native drawing inputs are prepared states; production receipts bind them to captured Rust state and original native stores. There is no native whole-Scenario trajectory or whole-frame comparison. Original UI dispatch is instruction evidence; Rust gestures execute the ordinary input owner.
- CPU/GPU parity is finite:120 producer inputs,6 mixed opaque orders,77 solid leaves,45 rectangles and two sRGB formats at four zooms. Five known one-pixel clipping controls remain explicitly bounded CPU presentation residuals, not exact-native GPU claims.
- Whole save/load reconstruction order is not established, although the process-timer reanchor is executed natively and connected to the production commit. Non-stock height fallbacks and extreme integer domains are outside these comparisons.
- Attack source/lead and admission, planning, other Foot/special destination paths, range/effect families and mixed-family interleaving remain separate required chains. The broad renderer cannot be certified from ordinary MTNK output alone.
- Performance evidence is a synthetic overlapping workload on the recorded M4/Metal host; it does not establish20k dispersed gameplay,30-player performance or Linux/Windows GPU behavior.

Owner disposition:

- Corrected the documented command to the module invocation. Executed that exact form with PYTHONPATH unset: native outputs and metadata match, no files written. Saved `unit-move-doc-command-check.log` in the validation receipt.
- Updated the ground_move module description to name the direct ordinary Unit destination dispatch and its repeated-NavCom admission. Comment-only change.
- No runtime implementation changes followed review; no second critic pass was requested. Whole-scope residuals remain open.
