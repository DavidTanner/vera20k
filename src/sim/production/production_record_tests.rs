//! The production family's native records (Networking stage 2b): each
//! command a local producer issues encodes to the record its native producer
//! sends, and the record decodes back to a command whose receiver does what
//! the native receiver does.

use super::ProductionCategory;
use super::tests::{arm_build_via, spawn_structure};
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{
    COMMAND_RECORD_LEN, Command, CommandEnvelope, CommandRecord, PRIMARY_OPCODE, PlaceRecord,
    ProductionEvent, ProductionEventRecord, ProductionRtti, TargetEventRecord,
};
use crate::sim::house_state::HouseState;
use crate::sim::intern::InternedId;
use crate::sim::world::Simulation;

const FRAME: u32 = 77;

fn record_rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[Countries]\n0=Americans\n1=Russians\n\
         [InfantryTypes]\n0=E1\n1=E2\n\
         [VehicleTypes]\n0=MTNK\n1=DEST\n\
         [AircraftTypes]\n0=ORCA\n\
         [BuildingTypes]\n0=GAPILE\n1=GAWEAP\n2=GAYARD\n3=GAPOWR\n4=GAPILL\n\
         [E1]\nCost=200\nOwner=Americans\n\
         [E2]\nCost=300\nOwner=Americans\n\
         [MTNK]\nCost=700\nOwner=Americans\n\
         [DEST]\nCost=1000\nNaval=yes\nOwner=Americans\n\
         [ORCA]\nCost=1200\nOwner=Americans\n\
         [GAPILE]\nOwner=Americans\nFactory=InfantryType\n\
         [GAWEAP]\nOwner=Americans\nFactory=UnitType\n\
         [GAYARD]\nOwner=Americans\nFactory=UnitType\nNaval=yes\n\
         [GAPOWR]\nOwner=Americans\n\
         [GAPILL]\nOwner=Americans\nBuildCat=Combat\n",
    ))
    .expect("production record rules")
}

/// The local house is the second registration, so its record house byte is
/// its `ArrayIndex` 1, not the interner id.
fn record_world() -> (Simulation, RuleSet, InternedId) {
    let rules = record_rules();
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    for name in ["Russians", "Americans"] {
        let house = sim.intern(name);
        sim.houses
            .insert(house, HouseState::new(house, 0, None, true, 10_000, 10));
        sim.session.house_order.push(house);
    }
    sim.session.binary_frame = FRAME;
    let owner = sim.intern("Americans");
    (sim, rules, owner)
}

fn type_id(sim: &Simulation, name: &str) -> InternedId {
    sim.interner.get(name).unwrap()
}

fn roundtrip(sim: &Simulation, rules: &RuleSet, owner: InternedId, command: &Command) -> Command {
    let record = sim
        .encode_production_record(owner, command, rules)
        .unwrap_or_else(|| panic!("{command:?} encodes"));
    let envelope = sim
        .decode_native_command_record(&record, Some(rules), 5)
        .unwrap_or_else(|| panic!("{command:?} decodes"));
    assert_eq!((envelope.owner, envelope.execute_tick), (owner, 5));
    envelope.payload
}

/// SelectClass::Action (`0x006AAD00`) sends PRODUCE, ABANDON and
/// ABANDON_ALL with the strip's type RTTI (`+0x5C`), its type index (`+0x58`)
/// and the type's `Naval=`, as Strip Recalculate (`0x006AA7E7`,
/// `0x006AA8CD`) sends its ABANDONs.
#[test]
fn cameo_events_carry_the_type_rtti_index_and_naval_flag() {
    let (sim, rules, owner) = record_world();
    let cases = [
        (
            "E2",
            ProductionEvent::Produce,
            ProductionRtti::InfantryType,
            1,
            false,
        ),
        (
            "DEST",
            ProductionEvent::Produce,
            ProductionRtti::UnitType,
            1,
            true,
        ),
        (
            "ORCA",
            ProductionEvent::Produce,
            ProductionRtti::AircraftType,
            0,
            false,
        ),
        (
            "GAPILL",
            ProductionEvent::Produce,
            ProductionRtti::BuildingType,
            4,
            false,
        ),
        (
            "MTNK",
            ProductionEvent::Abandon,
            ProductionRtti::UnitType,
            0,
            false,
        ),
        (
            "GAPOWR",
            ProductionEvent::AbandonAll,
            ProductionRtti::BuildingType,
            3,
            false,
        ),
    ];
    for (name, event, rtti, type_index, naval) in cases {
        let type_id = type_id(&sim, name);
        let command = match event {
            ProductionEvent::Produce => Command::QueueProduction { type_id },
            ProductionEvent::Abandon => Command::CancelProductionByType {
                type_id,
                all: false,
            },
            _ => Command::CancelProductionByType { type_id, all: true },
        };
        let record = sim
            .encode_production_record(owner, &command, &rules)
            .unwrap();
        assert_eq!(
            ProductionEventRecord::decode(&record),
            Some(ProductionEventRecord {
                event,
                house_id: 1,
                frame: FRAME as i32,
                rtti,
                type_index,
                naval,
            }),
            "{name}"
        );
        assert_eq!(roundtrip(&sim, &rules, owner, &command), command, "{name}");
    }
}

/// SUSPEND names the clicked cameo's type, the one its factory builds; the
/// receiver (`HouseClass::Suspend_Production @ 0x004FA910`) reads only the
/// slot that type selects.
#[test]
fn suspend_names_the_building_type_and_decodes_to_its_slot() {
    let (mut sim, rules, owner) = record_world();
    let suspend = |category| Command::SuspendProduction { category };
    assert_eq!(
        sim.encode_production_record(owner, &suspend(ProductionCategory::Ship), &rules),
        None,
        "nothing in production names no type"
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "DEST",
        ProductionCategory::Ship,
        1,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "GAPILL",
        ProductionCategory::Defense,
        2,
    );
    for (category, rtti, type_index, naval) in [
        (ProductionCategory::Ship, ProductionRtti::UnitType, 1, true),
        (
            ProductionCategory::Defense,
            ProductionRtti::BuildingType,
            4,
            false,
        ),
    ] {
        let record = sim
            .encode_production_record(owner, &suspend(category), &rules)
            .unwrap();
        let typed = ProductionEventRecord::decode(&record).unwrap();
        assert_eq!(typed.event, ProductionEvent::Suspend);
        assert_eq!(
            (typed.rtti, typed.type_index, typed.naval),
            (rtti, type_index, naval)
        );
        assert_eq!(
            roundtrip(&sim, &rules, owner, &suspend(category)),
            suspend(category)
        );
    }
}

/// A building's placement click (`DisplayClass::BandBox_LeftUp`
/// `0x004ABB55`) sends the placement object's RTTI (Building), the type's
/// index and the Cell; Strip AI (`0x006A8EE8`) sends a completed mobile's
/// RTTI, type index -1 and the empty Cell.
#[test]
fn place_events_take_the_placement_and_strip_forms() {
    let (sim, rules, owner) = record_world();
    let gapowr = type_id(&sim, "GAPOWR");
    for (rx, ry, x, y) in [(31, 7, 31, 7), (u16::MAX, u16::MAX, -1, -1), (0, 0, 0, 0)] {
        let command = Command::PlaceReadyBuilding {
            type_id: gapowr,
            rx,
            ry,
        };
        let record = sim
            .encode_production_record(owner, &command, &rules)
            .unwrap();
        assert_eq!(
            PlaceRecord::decode(&record),
            Some(PlaceRecord {
                house_id: 1,
                frame: FRAME as i32,
                rtti: ProductionRtti::Building,
                type_index: 3,
                naval: false,
                x,
                y,
            })
        );
        assert_eq!(roundtrip(&sim, &rules, owner, &command), command);
    }
    let mtnk = type_id(&sim, "MTNK");
    let not_a_building = Command::PlaceReadyBuilding {
        type_id: mtnk,
        rx: 1,
        ry: 1,
    };
    assert_eq!(
        sim.encode_production_record(owner, &not_a_building, &rules),
        None
    );

    for (category, rtti, naval) in [
        (
            ProductionCategory::Infantry,
            ProductionRtti::Infantry,
            false,
        ),
        (ProductionCategory::Vehicle, ProductionRtti::Unit, false),
        (ProductionCategory::Ship, ProductionRtti::Unit, true),
        (
            ProductionCategory::Aircraft,
            ProductionRtti::Aircraft,
            false,
        ),
    ] {
        let command = Command::PlaceProducedMobile { category };
        let record = sim
            .encode_production_record(owner, &command, &rules)
            .unwrap();
        assert_eq!(
            PlaceRecord::decode(&record),
            Some(PlaceRecord {
                house_id: 1,
                frame: FRAME as i32,
                rtti,
                type_index: -1,
                naval,
                x: 0,
                y: 0,
            })
        );
        assert_eq!(roundtrip(&sim, &rules, owner, &command), command);
    }
    for category in [ProductionCategory::Building, ProductionCategory::Defense] {
        assert_eq!(
            sim.encode_production_record(owner, &Command::PlaceProducedMobile { category }, &rules),
            None,
            "Strip AI sends no mobile PLACE for a building"
        );
    }
}

fn record_with(write: impl FnOnce(&mut CommandRecord)) -> CommandRecord {
    let mut record = CommandRecord::decode_exact(&[0; COMMAND_RECORD_LEN]).unwrap();
    write(&mut record);
    record
}

/// Forms no producer sends, which VERA's receivers cannot represent, decode
/// to nothing; the other native forms decode to their receiver's command.
#[test]
fn production_decoder_takes_only_represented_forms() {
    let (sim, rules, owner) = record_world();
    let decode = |record: &CommandRecord| {
        sim.decode_native_command_record(record, Some(&rules), 5)
            .map(|envelope| envelope.payload)
    };
    let produce = |rtti, type_index, naval| {
        record_with(|record| {
            ProductionEventRecord {
                event: ProductionEvent::Produce,
                house_id: 1,
                frame: 9,
                rtti,
                type_index,
                naval,
            }
            .write_into(record)
        })
    };
    let place = |rtti, type_index, x, y| {
        record_with(|record| {
            PlaceRecord {
                house_id: 1,
                frame: 9,
                rtti,
                type_index,
                naval: false,
                x,
                y,
            }
            .write_into(record)
        })
    };
    let mtnk = type_id(&sim, "MTNK");
    // The receivers accept the object RTTI as well as the type RTTI.
    assert_eq!(
        decode(&produce(ProductionRtti::Unit, 0, false)),
        Some(Command::QueueProduction { type_id: mtnk })
    );
    // A Unit's naval dword picks the factory slot; VERA's receiver takes the
    // slot from the type.
    assert_eq!(decode(&produce(ProductionRtti::UnitType, 0, true)), None);
    assert_eq!(decode(&produce(ProductionRtti::UnitType, 2, false)), None);
    assert_eq!(decode(&produce(ProductionRtti::UnitType, -1, false)), None);
    // SelectClass's click on a completed mobile cameo (`0x006AB3FC`) sends
    // the strip's type RTTI with index -1.
    assert_eq!(
        decode(&place(ProductionRtti::UnitType, -1, -1, -1)),
        Some(Command::PlaceProducedMobile {
            category: ProductionCategory::Vehicle
        })
    );
    assert_eq!(decode(&place(ProductionRtti::Unit, -1, 4, 4)), None);
    assert_eq!(decode(&place(ProductionRtti::Unit, 0, 0, 0)), None);
    assert_eq!(decode(&place(ProductionRtti::Building, -1, 0, 0)), None);
    // Without the match's rules no type index can be resolved.
    let record = sim
        .encode_production_record(owner, &Command::QueueProduction { type_id: mtnk }, &rules)
        .unwrap();
    assert_eq!(sim.decode_native_command_record(&record, None, 5), None);
    // A Special (`0x1F`) strip entry is not a production RTTI.
    let mut special = record.clone();
    special.payload_mut()[..4].copy_from_slice(&0x1f_i32.to_le_bytes());
    assert_eq!(decode(&special), None);
}

fn factory_world() -> (Simulation, RuleSet, InternedId) {
    let (mut sim, rules, owner) = record_world();
    for (id, cell) in [(3, (10, 10)), (5, (15, 15)), (9, (20, 20))] {
        spawn_structure(&mut sim, &rules, id, "Americans", "GAWEAP", cell.0, cell.1);
    }
    (sim, rules, owner)
}

/// The Factory button sends the chosen building's PRIMARY (Building
/// ClickedAction `0x00443494`, constructor `0x004C65E0`), and the decoded
/// TogglePrimaryFactory leaves the same state the semantic command does.
#[test]
fn producer_focus_sends_primary_and_matches_the_semantic_cycle() {
    let (mut semantic, rules, owner) = factory_world();
    let (mut decoded_world, _, _) = factory_world();
    let cycle = Command::CycleProducerFocus {
        category: ProductionCategory::Vehicle,
    };
    for expected in [3, 5, 9, 3] {
        let record = decoded_world
            .encode_production_record(owner, &cycle, &rules)
            .unwrap();
        assert_eq!(
            TargetEventRecord::decode(&record),
            Some(TargetEventRecord {
                opcode: PRIMARY_OPCODE,
                house_id: 1,
                frame: FRAME as i32,
                entity_id: expected,
            })
        );
        let decoded = decoded_world
            .decode_native_command_record(&record, Some(&rules), 5)
            .unwrap();
        assert_eq!(
            decoded,
            CommandEnvelope::new(
                owner,
                5,
                Command::TogglePrimaryFactory {
                    building_id: expected as u64
                }
            )
        );
        assert!(semantic.apply_command("Americans", &cycle, Some(&rules)));
        assert!(decoded_world.apply_command("Americans", &decoded.payload, Some(&rules)));
        for sim in [&semantic, &decoded_world] {
            assert_eq!(
                sim.production
                    .primary_factory(owner, ProductionCategory::Vehicle),
                Some(expected as u64)
            );
        }
        assert_eq!(semantic.state_hash(), decoded_world.state_hash());
    }
}

/// The PRIMARY receiver toggles a primary factory off, and names no house:
/// it toggles whatever live building the record names (`EventClass::Execute`
/// PRIMARY arm, `TogglePrimaryFactory @ 0x00448182`).
#[test]
fn primary_record_toggles_the_named_building() {
    let (mut sim, rules, owner) = factory_world();
    let toggle = Command::TogglePrimaryFactory { building_id: 5 };
    assert_eq!(roundtrip(&sim, &rules, owner, &toggle), toggle);
    assert!(sim.apply_command("Russians", &toggle, Some(&rules)));
    assert_eq!(
        sim.production
            .primary_factory(owner, ProductionCategory::Vehicle),
        Some(5)
    );
    assert!(sim.apply_command("Russians", &toggle, Some(&rules)));
    assert_eq!(
        sim.production
            .primary_factory(owner, ProductionCategory::Vehicle),
        None
    );
}

/// Each primary slot is its own: a naval yard and a war factory are both
/// primary, and a building without `Factory=` or no building is no PRIMARY
/// receiver. A player's house hears `EVA_PrimaryBuildingSelected`.
#[test]
fn primary_toggle_keeps_slots_apart_and_skips_non_factories() {
    let (mut sim, rules, owner) = factory_world();
    spawn_structure(&mut sim, &rules, 7, "Americans", "GAYARD", 30, 30);
    spawn_structure(&mut sim, &rules, 11, "Americans", "GAPOWR", 40, 40);
    sim.sound_events.clear();
    assert!(super::toggle_primary_factory(&mut sim, &rules, 7));
    assert!(super::toggle_primary_factory(&mut sim, &rules, 3));
    assert_eq!(
        sim.production
            .primary_factory(owner, ProductionCategory::Ship),
        Some(7)
    );
    assert_eq!(
        sim.production
            .primary_factory(owner, ProductionCategory::Vehicle),
        Some(3)
    );
    let primary_lines = sim
        .sound_events
        .iter()
        .filter(|event| {
            matches!(event, crate::sim::world::SimSoundEvent::HouseEva {
                owner: house,
                event: "EVA_PrimaryBuildingSelected",
            } if *house == owner)
        })
        .count();
    assert_eq!(primary_lines, 2);
    assert!(!super::toggle_primary_factory(&mut sim, &rules, 11));
    assert!(!super::toggle_primary_factory(&mut sim, &rules, 99));
    assert_eq!(
        sim.encode_production_record(
            owner,
            &Command::TogglePrimaryFactory { building_id: 99 },
            &rules
        ),
        None
    );
}

/// Every type of the four retail TechnoType arrays, through the production
/// reader on the retail `rulesmd.ini`, encodes by its own array index and
/// decodes to the same type: no name resolves into another array, and the
/// naval Units keep their naval dword.
#[test]
fn retail_types_round_trip_through_their_array_index() {
    use crate::rules::object_type::ObjectCategory;
    let Some((ini, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    let owner = sim.intern("Americans");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, true, 0, 10));
    sim.session.house_order.push(owner);
    let mut naval = 0;
    let mut checked = 0;
    for category in [
        ObjectCategory::Infantry,
        ObjectCategory::Vehicle,
        ObjectCategory::Aircraft,
        ObjectCategory::Building,
    ] {
        for (index, name) in rules.type_array_ids(category).iter().enumerate() {
            let index = i32::try_from(index).unwrap();
            let Some(object) = rules.type_array_at(category, index) else {
                continue;
            };
            let command = Command::QueueProduction {
                type_id: type_id(&sim, name),
            };
            let record = sim
                .encode_production_record(owner, &command, &rules)
                .unwrap_or_else(|| panic!("{name} encodes"));
            let typed = ProductionEventRecord::decode(&record).unwrap();
            assert_eq!(
                (typed.rtti, typed.type_index, typed.naval),
                (ProductionRtti::type_of(category), index, object.naval),
                "{name}"
            );
            assert_eq!(roundtrip(&sim, &rules, owner, &command), command, "{name}");
            naval += usize::from(object.naval && category == ObjectCategory::Vehicle);
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked} retail types");
    assert!(naval > 0, "retail has naval Units");
}
