//! UpdateAll against `tools/superweapon_oracle.json` `ion_blast_update` (the
//! original `0x0053D310` run natively over flagged blasts).

use super::*;
use serde_json::Value;

/// Each row's blasts, in vector order, update once: the ones left keep their
/// order, their frames advance, and a frame of 79 or more is removed.
#[test]
fn update_all_matches_native() {
    let oracle: Value =
        serde_json::from_str(crate::test_fixture::text("tools/superweapon_oracle.json")).unwrap();
    let rows = oracle["ion_blast_update"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let mut sim = Simulation::new();
        // The X coordinate carries each blast's index in the row.
        for (index, frame) in row["frames"].as_array().unwrap().iter().enumerate() {
            sim.ion_blasts.push(IonBlast {
                coord: [index as i32, 0, 0],
                frame: frame.as_i64().unwrap() as i32,
            });
        }
        sim.update_ion_blasts();
        let left: Vec<[i64; 2]> = row["left"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| [pair[0].as_i64().unwrap(), pair[1].as_i64().unwrap()])
            .collect();
        let ours: Vec<[i64; 2]> = sim
            .ion_blasts()
            .map(|(coord, frame)| [i64::from(coord[0]), i64::from(frame)])
            .collect();
        assert_eq!(ours, left, "{row}");
    }
}
