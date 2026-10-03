# Combined main1024 / CLEG hash attribution

The same Cargo-owned test binary executes the global replay with the temporary
absent Teleport zero tag enabled and disabled. The fixture keeps incoming main's
CDA10AEE6EA0AE46 baseline during both runs. The control passes; the current run
reaches only the expected final baseline assertion, reporting534C7788BB49A3D8.
All601 complete observations are equal after removing only tick_result.state_hash.
This is Rust hash-composition attribution, not native gameplay parity.

Run `python tools/spatial_oracle/infantry_teleport_replay/main1024/strict-row-check.py`
to independently check the retained rows. To regenerate in an isolated checkout,
apply the archived four-line test-only control at the recorded world hash slot,
keep the incoming baseline in the harness, then run the named lib test first with
VERA20K_CLEG_ABSENT_TAG_CONTROL=1 and then without it, setting separate
VERA20K_REPLAY_DIAGNOSTICS directories. Use the shared Cargo owner. Remove the
control before validation. The archived producer is the actual run, including
its obsolete assertion-message check; receipt.json records that collector failure
and the strict saved-row comparison that resolved it. No gameplay assertion or
native golden was changed.
