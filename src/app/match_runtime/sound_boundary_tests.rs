//! Headless app-boundary transport checks. These exercise the production
//! dispatcher and its local/radar gates; they do not establish device parity.

use super::{FrameSoundBoundary, dispatch_sim_sound_events};
use crate::audio::events::SoundEventQueue;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::radar::{RadarEventRequest, RadarEventType};
use crate::sim::world::{SimSoundEvent, Simulation, SoundBoundary};

#[test]
fn frame_boundary_consumes_each_prefix_once_and_preserves_later_facts() {
    let rules = RuleSet::from_ini(&IniFile::from_str("")).unwrap();
    let mut sim = Simulation::new();
    sim.session.tick = 17;
    let local = sim.interner.intern("Local");
    let remote = sim.interner.intern("Remote");
    let before_main = sim.rng_views().main.native_state_hex();
    let repaired = |owner, rx| SimSoundEvent::UnitRepaired {
        owner,
        radar: RadarEventRequest::new(RadarEventType::UnitRepaired, rx, 7),
    };
    let mut facts = vec![repaired(remote, 1), repaired(local, 2), repaired(local, 99)];
    let mut output = SoundEventQueue::new();
    let mut admitted = Vec::new();
    let delivered;
    {
        let mut admit_radar = |request: RadarEventRequest, tick, bound_rules: &RuleSet| {
            assert!(std::ptr::eq(bound_rules, &rules));
            admitted.push((tick, request.rx));
            request.rx != 99
        };
        // Absent output is the ordinary headless/audio-unavailable path.
        // Cue/radar conversion still runs through the production dispatcher.
        let boundary = FrameSoundBoundary::new(None, Some("LOCAL"), &mut output, &mut admit_radar);
        boundary.consume(&sim, &rules, &facts);
        boundary.consume(&sim, &rules, &facts);
        facts.push(SimSoundEvent::UnitVoiceVisit { owner: 41 });
        boundary.consume(&sim, &rules, &facts);
        facts.push(repaired(local, 3));
        boundary.consume(&sim, &rules, &facts);
        delivered = boundary.delivered();
    }
    assert_eq!(admitted, [(17, 2), (17, 99), (17, 3)]);
    assert_eq!(delivered, 5);
    assert_eq!(facts.len(), 5, "inline audio must preserve the frame facts");
    assert!(matches!(
        facts[3],
        SimSoundEvent::UnitVoiceVisit { owner: 41 }
    ));
    assert!(
        output.is_empty(),
        "inline delivery drains the existing queue"
    );

    // The ordinary post-frame dispatcher takes only the unconsumed suffix.
    facts.push(repaired(local, 4));
    dispatch_sim_sound_events(
        facts.into_iter().skip(delivered),
        &sim,
        &rules,
        Some("LOCAL"),
        &mut |request| {
            admitted.push((sim.session.tick, request.rx));
            true
        },
        &mut output,
    );
    assert_eq!(admitted, [(17, 2), (17, 99), (17, 3), (17, 4)]);
    assert_eq!(output.drain().len(), 1);

    // A later frame starts a new borrowed scope, with no retained frontier.
    sim.session.tick = 18;
    {
        let mut admit_radar = |request: RadarEventRequest, tick, _: &RuleSet| {
            admitted.push((tick, request.rx));
            true
        };
        let boundary = FrameSoundBoundary::new(None, Some("LOCAL"), &mut output, &mut admit_radar);
        boundary.consume(&sim, &rules, &[repaired(local, 2)]);
        assert_eq!(boundary.delivered(), 1);
    }
    assert_eq!(admitted.last(), Some(&(18, 2)));
    assert_eq!(sim.rng_views().main.native_state_hex(), before_main);
}
