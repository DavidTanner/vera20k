# Two-vehicle route causality

These are Rust regression transcripts, not native whole-world goldens. The
original native downgrade and clearing-cost comparisons are in
`../../astar_hills_markers.json`. Both recorded executions used the same
compiled executable, SHA
`21a14c9e3c82b263a3c8d3cbd1c45e9eb04939c0099d9005d48fdfc659424835`.
The receipt records its captured source identity and the limits of source
readback. Gzip files retain the complete logs, including caller routes and
marker/finishing observations; hashes cover compressed and raw bytes.

The temporary test gate in `counterfactual.txt` changes only the effective
urgency returned by the single marker preparation owner. Keeping requested
urgency one gives main7412's first repath `[5,6,7,6,6,6,6]`, closest separation
127 at tick50, and no shared cells. Returning native effective urgency zero
gives `[6,5,7,6,6,6,6]`, closest separation110 at tick53, and one shared cell
at tick54 with separation116. Finishing leaves each reconstructed direction
array unchanged. The native behavior changes route selection upstream of
finishing, which selects different subsequent paid Drive curves.

Both executions occurred before the Rust regression threshold changed from119
to116. The normal execution therefore failed that historical threshold; the
counterfactual passed it. The visible-overlap floor108 and at-most-one-shared-
tick assertions remain unchanged. No native two-vehicle admission or paid
movement comparison is claimed by these numbers.

To reproduce the contrast, temporarily insert the supplied gate, build once
through `tools.cargo_run`, and execute the exact lib test
`sim::world::tests::repro_two_moving_vehicles_pass_through_each_other` with
`VERA20K_REQUIRE_RETAIL_INI=1` and retail inputs. Run once with
`VERA20K_DIAGNOSTIC_KEEP_REQUESTED_URGENCY=1`, once with it absent. The old
finishing diagnostic prints in these retained logs were removed after the
comparison; the fixture's route and separation output remains available.
Remove the temporary gate and validate the final source afterward.
