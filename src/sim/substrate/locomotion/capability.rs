//! Per-class locomotor capabilities that change native control flow.

use crate::rules::locomotor_type::LocomotorKind;

/// Whether a live class implements the native piggyback capability.
///
/// Five active-YR classes expose it. The sixth native provider is DropPod,
/// which is deliberately absent because stock YR never selects it.
pub const fn piggyback_capable(class: LocomotorKind) -> bool {
    matches!(
        class,
        LocomotorKind::Drive
            | LocomotorKind::Ship
            | LocomotorKind::Walk
            | LocomotorKind::Jumpjet
            | LocomotorKind::Teleport
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_live_classes_are_piggyback_capable() {
        let capable: Vec<_> = LocomotorKind::ALL
            .into_iter()
            .filter(|class| piggyback_capable(*class))
            .collect();
        assert_eq!(
            capable,
            [
                LocomotorKind::Drive,
                LocomotorKind::Walk,
                LocomotorKind::Teleport,
                LocomotorKind::Ship,
                LocomotorKind::Jumpjet,
            ]
        );
    }
}
