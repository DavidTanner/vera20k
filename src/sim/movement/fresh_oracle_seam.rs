//! Test-only callee answers and call records for the Drive/Ship fresh-arm
//! replay of `tools/spatial_oracle/track_fresh_response`, which substitutes
//! the same four callees: Unit `Can_Enter_Cell` (0x73F0A0) and Foot
//! `Find_Path` (0x4D3920) answer from supplied queues; Cell `Scatter_Objects`
//! (0x481670) and `Foot::Override_Mission` (0x4D8F40) are recorded and, like
//! the oracle's substitutions, their bodies do not run. Nothing is supplied,
//! recorded or skipped unless a test installs the queues. Walk's response
//! corpus opts into live effects: the same boundaries are recorded while the
//! real Scatter and Override bodies run.

use std::cell::RefCell;
use std::collections::VecDeque;

use crate::sim::combat::TargetKind;

/// A supplied `Find_Path` answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SuppliedPath {
    /// AL = 1 after writing these words to Foot+5E0.
    Found(Vec<u8>),
    /// AL = 1 with no writes, as the Walk decoder's direct code-2 controls.
    FoundUnchanged,
    /// AL = 0 with no writes.
    Failed,
    /// The original wrapper runs; only its AStar core (0x4CBBA0) is NULL.
    CoreNull,
}

/// One substituted call, with the caller's arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FreshCallRecord {
    CanEnter {
        cell: (i16, i16),
        direction: i32,
        height: i32,
        code: u8,
    },
    FindPath {
        cell: (i32, i32),
        urgency: u8,
    },
    Scatter {
        cell: (i16, i16),
        no_kidding: bool,
        deck: bool,
    },
    Override {
        target: TargetKind,
    },
    UncloakContacts {
        cell: (i16, i16),
    },
}

#[derive(Default)]
struct Seam {
    codes: VecDeque<u8>,
    paths: VecDeque<SuppliedPath>,
    records: Vec<FreshCallRecord>,
    core_null: bool,
    live_effects: bool,
    live_can_enter: bool,
}

thread_local! {
    static SEAM: RefCell<Option<Seam>> = const { RefCell::new(None) };
}

/// Install the supplied answers for one replayed row.
pub(crate) fn install(codes: Vec<u8>, paths: Vec<SuppliedPath>) {
    SEAM.with(|seam| {
        *seam.borrow_mut() = Some(Seam {
            codes: codes.into(),
            paths: paths.into(),
            records: Vec::new(),
            core_null: false,
            live_effects: false,
            live_can_enter: false,
        })
    });
}

/// Walk's response corpus executes the real Scatter and Override bodies.
/// Keep recording their boundaries without substituting those effects.
pub(crate) fn install_with_live_effects(codes: Vec<u8>, paths: Vec<SuppliedPath>) {
    install(codes, paths);
    SEAM.with(|seam| seam.borrow_mut().as_mut().unwrap().live_effects = true);
}

/// A composed native comparison supplies only Find_Path's route, allowing
/// the real Unit admission owner to classify the marked world and occupants.
pub(crate) fn install_path_only(paths: Vec<SuppliedPath>) {
    install(Vec::new(), paths);
    SEAM.with(|seam| {
        let mut seam = seam.borrow_mut();
        let seam = seam.as_mut().unwrap();
        seam.live_can_enter = true;
        seam.live_effects = true;
    });
}

/// Remove the seam, returning the records and any unused answers.
pub(crate) fn finish() -> (Vec<FreshCallRecord>, usize) {
    SEAM.with(|seam| {
        seam.borrow_mut().take().map_or((Vec::new(), 0), |seam| {
            (
                seam.records,
                seam.codes.len() + seam.paths.len() + usize::from(seam.core_null),
            )
        })
    })
}

/// A `CoreNull` answer arms this for the core search of the same call.
pub(crate) fn arm_core_null() {
    SEAM.with(|seam| {
        if let Some(seam) = seam.borrow_mut().as_mut() {
            seam.core_null = true;
        }
    });
}

/// True once per armed `CoreNull`: the core search answers NULL.
pub(crate) fn take_core_null() -> bool {
    SEAM.with(|seam| {
        seam.borrow_mut()
            .as_mut()
            .is_some_and(|seam| std::mem::take(&mut seam.core_null))
    })
}

pub(crate) fn supplied_can_enter(cell: (i16, i16), direction: i32, height: i32) -> Option<u8> {
    SEAM.with(|seam| {
        let mut seam = seam.borrow_mut();
        let seam = seam.as_mut()?;
        if seam.live_can_enter {
            return None;
        }
        let code = seam.codes.pop_front().expect("unsupplied Can_Enter_Cell");
        seam.records.push(FreshCallRecord::CanEnter {
            cell,
            direction,
            height,
            code,
        });
        Some(code)
    })
}

pub(crate) fn supplied_path(cell: (i32, i32), urgency: u8) -> Option<SuppliedPath> {
    SEAM.with(|seam| {
        let mut seam = seam.borrow_mut();
        let seam = seam.as_mut()?;
        let path = seam.paths.pop_front().expect("unsupplied Find_Path");
        seam.records
            .push(FreshCallRecord::FindPath { cell, urgency });
        Some(path)
    })
}

/// Additional read/callback ordering in the live-effects Walk corpus.
pub(crate) fn observe_live(call: FreshCallRecord) {
    SEAM.with(|seam| {
        if let Some(seam) = seam.borrow_mut().as_mut()
            && seam.live_effects
        {
            seam.records.push(call);
        }
    });
}

/// Record a substituted call; true when a seam is installed, so the caller
/// skips the callee body.
pub(crate) fn substitute(call: FreshCallRecord) -> bool {
    SEAM.with(|seam| {
        seam.borrow_mut().as_mut().is_some_and(|seam| {
            seam.records.push(call);
            !seam.live_effects
        })
    })
}
