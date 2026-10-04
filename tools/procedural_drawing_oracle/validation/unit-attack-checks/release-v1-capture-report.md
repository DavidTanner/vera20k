# Attack release v1 production captures

**5/5 VALID actual captures** through the existing `tools.map_observation` owner, sequentially with `env -u RA2_DIR`. Label `procedural-unit-attack-production-v1`; source `ee20ca9feabda48be633df55845891801b8782d6`; executable SHA256 `4b4a83ceed0f074bab7dd0461c2bfef33f6e26a55c5842e8f43601eaad06e18c`. Native emulation was held until all captures ended. No tracked source/config/ref changes or Cargo runs were made.

| Capture | Frame | Final state hash | Timer / selected IDs | Frame mean ms |
|---|---:|---:|---|---:|
| `stationary-attack-inputs-v1` | 1286 | 17983893863271988632 | 21 / [1374] | 8.958740 |
| `stationary-enemy-band-control-inputs-v1` | 1286 | 17983893863271988632 | 24 / [] | 8.733002 |
| `moving-attack-inputs-v1` | 1298 | 13568668633787589161 | 9 / [1374] | 8.543020 |
| `moving-enemy-band-control-inputs-v1` | 1298 | 13568668633787589161 | 24 / [] | 8.522836 |
| `moving-attack-observer-off-v1` | 1298 | 13568668633787589161 | 9 / [1374] | 8.429045 |

The cadence values are the existing final last60 frame-boundary means. They include simulation, diagnostic observation, rendering and presentation; they are not GPU-only durations or ordinary FPS. One capture per variant does not establish a timing regression.

**Matched controls:** stationary and moving enemy-band controls queue only `Select[]`, preserve every actor/House/terrain observation (1,287 and 1,299 boundaries), and preserve the final deterministic state hash. The band deliberately reanchors the timer: 24 remaining versus 21/9 in the selected captures. Both actors remain active, health 300, without rocking. Real input receipts retain Select 1374 at 1281 and Attack 1374→1386 at 1282.

**Old-release preservation:** after removing only the new per-actor `action_line_inputs` fields, all observations and final states equal the old-release runs for both selected/control pairs. Stationary selected/control and moving control frames are byte-identical to old release. The shared `map_observation.compare_runs` comparison of old `moving-attack-v2` against new `moving-attack-observer-off-v1` returns the expected `MISMATCH` with exactly one difference, `frame.bytes`, and no errors. The shared owner also checks profile/config/contract bytes, full presentation clock, atlas metadata and all observations. See `release-v1-old-moving-comparison.json`.

**Observer nonmutation:** moving profiles differ only by `observe_action_line_inputs=true`. The on/off runs have identical frame bytes and surface format, all 1,299 observations after dropping only 7,794 diagnostic actor records, initial/final state, loaded session, map identity, lifecycle, camera, neutral input, atlas and full presentation clock. Only wall-clock cadence is excluded from render metadata equality. No simulation or presentation-clock field is excluded.

**Pixel and visual checks:** stationary pair differs at 332 tactical pixels, including 188 red pixels; moving pair 245, including 101 red pixels. No changes occur outside the tactical viewport. All four selected/control PNGs were inspected: terrain/actors match, selection marks vanish in controls, stationary endpoint stays on the target, and the moving endpoint visibly leads it. Native executable pixel comparison remains the parent/native owner’s step; these observations do not certify native parity by themselves.

**Final query inputs:**

- Stationary 1286: source 1374 `[7349,23680,416]`, Drive moving, speed 17, body 16384/turret 16383; target 1386 `[8832,23680,416]`, Drive not moving, speed 0, body/turret 49152. Source TarCom 1386 and NavCom Cell 34,92 exercise Attack priority.
- Moving 1298: source 1374 `[7500,23680,416]`, Drive moving, speed 5, applied fraction 19661, body 16384/turret 16383; target 1386 `[8451,23680,416]`, Drive moving, speed 17, applied fraction 65536, body 49152/turret 49153. Source TarCom 1386, NavCom null; target NavCom Cell 32,92.
- Both use current weapon 105mm, TurretOffset 0, veterancy 0, no rocking; house speed 1.0f32 and crate speed 1.0f64. Exact raw bits and receipts are retained in the JSON summary and source bundles.

Each capture directory contains sealed `profile.json`, `contract.json`, `config.toml`, `run.json`, logs and `child-output/{capture.json,frame.bgra}`. Same-stem `.png` files are outside those immutable bundles. `release-v1-capture-summary.json` retains exact frame/profile/capture/executable hashes, full final actor inputs, gestures, pair comparisons and observer projection. All files are under ignored `logs/procedural-drawing/attack-production/`.
