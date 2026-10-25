//! CR 406.3 + issue #10281: prenominal "exiled" / "face-up" type-phrase prefixes.
//! Cards: Pull from Eternity, Runic Repetition, Blade of the Swarm (warp).
//!
//! Uses public `parse_oracle_text` so this binary stays small (the in-crate
//! oracle_target unit suite OOMs memory-constrained builders).

use engine::parser::parse_oracle_text;
use engine::types::ability::{Effect, FilterProp, TargetFilter};
use engine::types::keywords::KeywordKind;
use engine::types::zones::Zone;

fn first_targeted_filter(oracle: &str, name: &str, types: &[&str]) -> TargetFilter {
    let types: Vec<String> = types.iter().map(|t| (*t).to_string()).collect();
    let parsed = parse_oracle_text(oracle, name, &[], &types, &[]);

    for ab in &parsed.abilities {
        if let Some(t) = effect_target(ab.effect.as_ref()) {
            return t;
        }
    }
    for trig in &parsed.triggers {
        if let Some(exec) = trig.execute.as_ref() {
            if let Some(t) = effect_target(exec.effect.as_ref()) {
                return t;
            }
            if let Some(sub) = exec.sub_ability.as_ref() {
                if let Some(t) = effect_target(sub.effect.as_ref()) {
                    return t;
                }
            }
        }
    }
    panic!(
        "{name}: no targeted effect in parse tree: abilities={:?} triggers={:?}",
        parsed.abilities, parsed.triggers
    );
}

fn effect_target(effect: &Effect) -> Option<TargetFilter> {
    match effect {
        Effect::ChangeZone { target, .. }
        | Effect::Bounce { target, .. }
        | Effect::TargetOnly { target } => Some(target.clone()),
        _ => None,
    }
}

fn assert_in_exile(filter: &TargetFilter, label: &str) {
    assert_eq!(
        filter.extract_in_zone(),
        Some(Zone::Exile),
        "{label}: extract_in_zone must be Exile, got {filter:?}"
    );
}

#[test]
fn pull_from_eternity_targets_face_up_exiled_card() {
    let filter = first_targeted_filter(
        "Return target face-up exiled card to its owner's graveyard.",
        "Pull from Eternity",
        &["Instant"],
    );
    assert_in_exile(&filter, "Pull from Eternity");
    let TargetFilter::Typed(tf) = &filter else {
        panic!("expected Typed, got {filter:?}");
    };
    assert!(
        tf.properties.iter().any(|p| matches!(
            p,
            FilterProp::Not { prop } if matches!(prop.as_ref(), FilterProp::FaceDown)
        )),
        "face-up must lower to Not{{FaceDown}}, got {:?}",
        tf.properties
    );
    assert!(
        tf.properties
            .contains(&FilterProp::InZone { zone: Zone::Exile }),
        "missing InZone Exile: {:?}",
        tf.properties
    );
}

#[test]
fn runic_repetition_targets_exiled_card_with_flashback() {
    let filter = first_targeted_filter(
        "Return target exiled card with flashback you own to your hand.",
        "Runic Repetition",
        &["Sorcery"],
    );
    assert_in_exile(&filter, "Runic Repetition");
    let TargetFilter::Typed(tf) = &filter else {
        panic!("expected Typed, got {filter:?}");
    };
    assert!(
        tf.properties.iter().any(|p| matches!(
            p,
            FilterProp::HasKeywordKind {
                value: KeywordKind::Flashback
            }
        )),
        "missing flashback: {:?}",
        tf.properties
    );
}

#[test]
fn face_up_exiled_card_with_warp_keeps_exile_and_warp() {
    let filter = first_targeted_filter(
        "Return target face-up exiled card with warp to its owner's hand.",
        "Warp Return Stand-in",
        &["Instant"],
    );
    assert_in_exile(&filter, "warp stand-in");
    let TargetFilter::Typed(tf) = &filter else {
        panic!("expected Typed, got {filter:?}");
    };
    assert!(
        tf.properties.iter().any(|p| matches!(
            p,
            FilterProp::HasKeywordKind {
                value: KeywordKind::Warp
            }
        )),
        "missing warp: {:?}",
        tf.properties
    );
    assert!(
        tf.properties.iter().any(|p| matches!(
            p,
            FilterProp::Not { prop } if matches!(prop.as_ref(), FilterProp::FaceDown)
        )),
        "face-up must lower to Not{{FaceDown}}, got {:?}",
        tf.properties
    );
}

#[test]
fn target_exiled_card_alone_keeps_exile_zone() {
    let filter = first_targeted_filter(
        "Put target exiled card into its owner's graveyard.",
        "Exiled Card Stand-in",
        &["Instant"],
    );
    assert_in_exile(&filter, "exiled card alone");
}
