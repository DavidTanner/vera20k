//! The single-event family's native records (Networking stage 2c): each
//! command a local producer issues encodes to the record its native producer
//! sends, and the record decodes back to a command whose receiver does what
//! the native receiver does.

use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{
    ARCHIVE_OPCODE, Command, CommandRecord, DEPLOY_OPCODE, GAMESPEED_OPCODE, IDLE_OPCODE,
    REPAIR_OPCODE, SELL_OPCODE, SPECIAL_PLACE_OPCODE,
};
use crate::sim::docking::bunker_link::BunkerLink;
use crate::sim::house_state::HouseState;
use crate::sim::intern::InternedId;
use crate::sim::world::Simulation;

const FRAME: u32 = 77;

fn record_rules() -> RuleSet {
    RuleSet::from_ini_with_fixed_art_for_test(
        &IniFile::from_str(
            "[Countries]\n0=Americans\n1=Russians\n\
             [InfantryTypes]\n0=GI\n\
             [VehicleTypes]\n0=AMCV\n1=APC\n2=MTNK\n3=HARV\n\
             [AircraftTypes]\n0=ORCA\n\
             [BuildingTypes]\n0=GACNST\n1=GAWEAP\n2=GABUNK\n\
             [SuperWeaponTypes]\n0=NukeSpecial\n1=LightningStormSpecial\n\
             [NukeSpecial]\nAction=Nuke\n[LightningStormSpecial]\nAction=LightningStorm\n\
             [GI]\nStrength=125\nSpeed=4\nDeployer=yes\nDeployFire=yes\n\
             [AMCV]\nStrength=450\nSpeed=5\nDeploysInto=GACNST\n\
             [APC]\nStrength=300\nSpeed=6\nPassengers=5\n\
             [MTNK]\nStrength=300\nSpeed=6\n\
             [HARV]\nStrength=600\nSpeed=4\nHarvester=yes\nDeploysInto=GACNST\n\
             [ORCA]\nStrength=200\nSpeed=12\n\
             [GACNST]\nStrength=1000\nFactory=BuildingType\nUndeploysInto=AMCV\n\
             [GAWEAP]\nStrength=1000\nFactory=UnitType\n\
             [GABUNK]\nStrength=1000\nBunker=yes\n\
             [Clear]\nBuildable=yes\n",
        ),
        &IniFile::from_str(
            "[GACNST]\nFoundation=4x3\n[GAWEAP]\nFoundation=3x2\n[GABUNK]\nFoundation=2x2\n",
        ),
    )
    .expect("single-event record rules")
}

/// The local house is the second registration, so its record house byte is
/// its `ArrayIndex` 1, not the interner id.
pub(crate) fn record_world() -> (Simulation, RuleSet, InternedId) {
    let rules = record_rules();
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
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

pub(crate) fn spawn(sim: &mut Simulation, rules: &RuleSet, type_id: &str, rx: u16, ry: u16) -> u64 {
    sim.spawn_object(type_id, "Americans", rx, ry, 0, rules)
        .unwrap_or_else(|| panic!("{type_id} spawns"))
}

fn decode(sim: &Simulation, rules: Option<&RuleSet>, record: CommandRecord) -> Option<Command> {
    sim.decode_native_command_record(&record, rules, 5)
        .map(|envelope| envelope.payload)
}

/// Every command of the family encodes to its producer's record, from the
/// local house's registration, and decodes back to itself.
#[test]
fn single_event_records_decode_to_the_issued_command() {
    let (mut sim, rules, owner) = record_world();
    let tank = spawn(&mut sim, &rules, "MTNK", 10, 10);
    let mcv = spawn(&mut sim, &rules, "AMCV", 12, 10);
    let gi = spawn(&mut sim, &rules, "GI", 14, 10);
    let apc = spawn(&mut sim, &rules, "APC", 16, 10);
    let factory = spawn(&mut sim, &rules, "GAWEAP", 20, 20);
    let second = spawn(&mut sim, &rules, "GAWEAP", 26, 20);
    let storm = sim.interner.get("LightningStormSpecial").unwrap();
    let cases = [
        (Command::Stop { entity_id: tank }, vec![IDLE_OPCODE]),
        (Command::DeployMcv { entity_id: mcv }, vec![DEPLOY_OPCODE]),
        (
            Command::ToggleInfantryDeploy { entity_id: gi },
            vec![DEPLOY_OPCODE],
        ),
        (
            Command::UnloadPassengers { transport_id: apc },
            vec![DEPLOY_OPCODE],
        ),
        (
            Command::SellBuilding { entity_id: factory },
            vec![SELL_OPCODE],
        ),
        (
            Command::ToggleRepair { entity_id: factory },
            vec![REPAIR_OPCODE],
        ),
        (
            Command::LaunchSuperWeapon {
                sw_type_id: storm,
                target_rx: 30,
                target_ry: 40,
            },
            vec![SPECIAL_PLACE_OPCODE],
        ),
        (Command::SetGameSpeed { speed: 4 }, vec![GAMESPEED_OPCODE]),
        (
            Command::SetRally {
                rx: 31,
                ry: 41,
                producer_ids: vec![factory, second],
            },
            vec![ARCHIVE_OPCODE, ARCHIVE_OPCODE],
        ),
    ];
    for (command, opcodes) in cases {
        let records = sim
            .encode_single_event_records(owner, &command, Some(&rules))
            .unwrap_or_else(|| panic!("{command:?} encodes"));
        assert_eq!(
            records
                .iter()
                .map(CommandRecord::opcode)
                .collect::<Vec<_>>(),
            opcodes,
            "{command:?}"
        );
        assert!(records.iter().all(|record| record.house_id() == 1
            && record.frame_stamp() == FRAME as i32
            && record.flags() == 0));
        let decoded: Vec<_> = records
            .into_iter()
            .map(|record| decode(&sim, Some(&rules), record).unwrap())
            .collect();
        let expected = match command {
            Command::SetRally {
                rx,
                ry,
                producer_ids,
            } => producer_ids
                .into_iter()
                .map(|id| Command::SetRally {
                    rx,
                    ry,
                    producer_ids: vec![id],
                })
                .collect(),
            command => vec![command],
        };
        assert_eq!(decoded, expected);
    }
    // SPECIAL_PLACE names the type's `[SuperWeaponTypes]` index, and decodes
    // only with the rules that list it.
    let launch = Command::LaunchSuperWeapon {
        sw_type_id: storm,
        target_rx: 30,
        target_ry: 40,
    };
    let [record] = sim
        .encode_single_event_records(owner, &launch, Some(&rules))
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(&record.payload()[..8], &[1, 0, 0, 0, 30, 0, 40, 0]);
    assert_eq!(decode(&sim, None, record), None);
    assert_eq!(sim.encode_single_event_records(owner, &launch, None), None);
}

/// DEPLOY queues Unload for every class but Aircraft (`0x004C7781`); the
/// class's Mission_Unload chooses the receiver. A self-click's MEGAMISSION
/// Unload reaches the same choice.
#[test]
fn deploy_and_self_click_unload_pick_the_class_unload_receiver() {
    let (mut sim, rules, owner) = record_world();
    let mcv = spawn(&mut sim, &rules, "AMCV", 10, 10);
    let tank = spawn(&mut sim, &rules, "MTNK", 12, 10);
    let harvester = spawn(&mut sim, &rules, "HARV", 14, 10);
    let orca = spawn(&mut sim, &rules, "ORCA", 16, 10);
    let bunker = spawn(&mut sim, &rules, "GABUNK", 20, 20);
    let occupant = spawn(&mut sim, &rules, "MTNK", 24, 10);
    sim.substrate
        .entities
        .get_mut(occupant)
        .unwrap()
        .bunker
        .set_bunker_link_for_test(BunkerLink::Installed(bunker));
    sim.substrate
        .entities
        .get_mut(bunker)
        .unwrap()
        .bunker
        .set_bunker_occupant_for_test(Some(occupant));
    let deploy = |sim: &Simulation, id, rules: Option<&RuleSet>| {
        let record = sim
            .encode_single_event_records(owner, &Command::DeployMcv { entity_id: id }, rules)
            .unwrap()
            .pop()
            .unwrap();
        decode(sim, rules, record)
    };
    let unload = |id| Command::UnloadPassengers { transport_id: id };
    assert_eq!(
        deploy(&sim, mcv, Some(&rules)),
        Some(Command::DeployMcv { entity_id: mcv })
    );
    // A Harvester= type takes the refinery-unload arm before DeploysInto=.
    assert_eq!(
        deploy(&sim, harvester, Some(&rules)),
        Some(unload(harvester))
    );
    assert_eq!(deploy(&sim, tank, Some(&rules)), Some(unload(tank)));
    assert_eq!(deploy(&sim, bunker, Some(&rules)), Some(unload(bunker)));
    assert_eq!(
        deploy(&sim, occupant, Some(&rules)),
        Some(Command::EjectBunker { bunker_id: bunker })
    );
    assert_eq!(deploy(&sim, orca, Some(&rules)), None);
    // Without rules a Unit's type is unknown: the cargo receiver.
    assert_eq!(deploy(&sim, mcv, None), Some(unload(mcv)));

    // The self-click form: Foot action 4's Unload16 with a NULL target. The
    // bunker eject click sends the occupant's.
    let self_click = |command: Command| {
        let record = sim.encode_megamission_record(owner, &command).unwrap();
        decode(&sim, Some(&rules), record)
    };
    assert_eq!(
        self_click(Command::DeployMcv { entity_id: mcv }),
        Some(Command::DeployMcv { entity_id: mcv })
    );
    let record = sim
        .encode_megamission_record(owner, &Command::EjectBunker { bunker_id: bunker })
        .unwrap();
    let typed = crate::sim::command::MegaMissionRecord::decode(&record).unwrap();
    assert_eq!(typed.source_id as u64, occupant);
    assert_eq!(
        typed.order,
        crate::sim::command::MegaMissionOrder::Unload {
            target: crate::sim::command::MegaMissionTarget::Null
        }
    );
    assert_eq!(
        decode(&sim, Some(&rules), record),
        Some(Command::EjectBunker { bunker_id: bunker })
    );
}

/// IDLE (`0x004C74CB`) and REPAIR (`0x004C6ED2`) read no house; SELL
/// (`0x004C6F12`) requires the object's owner to be the event's house.
#[test]
fn idle_and_repair_receivers_read_no_house_and_sell_does() {
    let (mut sim, rules, _) = record_world();
    let factory = spawn(&mut sim, &rules, "GAWEAP", 20, 20);
    let tank = spawn(&mut sim, &rules, "MTNK", 10, 10);
    let other = sim.interner.get("Russians").unwrap();
    let decoded = |sim: &Simulation, command: &Command| {
        let record = sim
            .encode_single_event_records(other, command, Some(&rules))
            .unwrap()
            .pop()
            .unwrap();
        let envelope = sim
            .decode_native_command_record(&record, Some(&rules), 5)
            .unwrap();
        assert_eq!(envelope.owner, other);
        envelope.payload
    };
    let repair = decoded(&sim, &Command::ToggleRepair { entity_id: factory });
    assert!(sim.apply_command("Russians", &repair, Some(&rules)));
    assert!(
        sim.substrate
            .entities
            .get(factory)
            .unwrap()
            .repairing
            .is_on()
    );
    let stop = decoded(&sim, &Command::Stop { entity_id: tank });
    assert!(sim.apply_command("Russians", &stop, Some(&rules)));
    let sell = decoded(&sim, &Command::SellBuilding { entity_id: factory });
    assert!(!sim.apply_command("Russians", &sell, Some(&rules)));
}
