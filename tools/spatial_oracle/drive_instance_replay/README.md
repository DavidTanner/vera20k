# Snapshot 288 retained Drive/Ship hash attribution

This is bounded Rust replay attribution, not native replay parity. The complete
class payload now owns its retained Option hash. Restoring only the old two
entity-level feeds recovers incoming 534C7788BB49A3D8; the current fold gives
B966DA8021F08904. The same test binary produced 601 complete rows per mode, equal
after removing only tick_result.state_hash. Existing gameplay and all 3 RNG
tripwires passed. The current as-run test fails only the old hash pin; the control
passes. The temporary control and old pin are removed from production source.

The existing checker owner is ../infantry_teleport_replay/main1024/strict-row-check.py.
Run that owner with `--directory tools/spatial_oracle/drive_instance_replay` from
the checkout. Its optional `--replay` and `--rows` arguments support the other
bounded replays; the default preserves the original global/601-row check.
receipt.json retains the same final_hashes contract.

For an original-run reproduction, apply control.patch.gz to the final candidate.
Run the named lib test from receipt.json with VERA20K_REPLAY_DIAGNOSTICS pointing
to a fresh absolute directory, first without VERA20K_DRIVE_INSTANCE_LEGACY_HASH,
then with it set to 1. The recorded old pin gives exits 101/0. Remove the patch after
the comparison. All Cargo commands use python -m tools.cargo_run.

The same control was also executed for Bridge (201 observations) and Slice6
(17 observations), recovering incoming 514E76C3355546F9 and 43BC438F280E7D17.
Their current folds are 4C4F372E184105B6 and E28ABFC8CA123369. Each complete pair
agrees except state_hash, including all three RNG observations, in the same executable.
Receipts/rows are in `other/bridge/` and `other/slice6/`; common as-run logs, source
freeze and actual binary SHA are in `other/`.

Check these with the same owner using `--directory` for their folder, `--replay`
bridge or slice6 and `--rows` 201 or 17. `other/control-as-run.patch.gz` preserves
the original patch. `other/control-reproduction.patch.gz` additionally restores
both incoming pins on the final source, reproducing the original 101/0 outcomes.
Remove it after comparison. These are bounded Rust attributions.
