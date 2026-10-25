//! Issue #9624 — Danny Pink self-CounterAdded "for the first time each turn".
//! Standalone integration target so Gate A can run without linking the full
//! `--lib` suite (link OOMs on small boxes).

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    ContinuousModification, TargetFilter, TriggerCondition, TriggerConstraint,
};
use engine::types::triggers::TriggerMode;

#[test]
fn danny_pink_self_counter_first_time_is_object_condition() {
    let parsed = parse_oracle_text(
        "Whenever one or more counters are put on this creature for the first time each turn, draw a card.",
        "Danny Pink",
        &[],
        &["Creature".to_string()],
        &[],
    );
    let def = parsed.triggers.first().expect("Danny Pink trigger");
    assert_eq!(def.mode, TriggerMode::CounterAdded);
    assert!(def.batched);
    assert_eq!(def.valid_card, Some(TargetFilter::SelfRef));
    assert_eq!(
        def.condition,
        Some(TriggerCondition::FirstTimeObjectCountersAddedThisTurn),
    );
    assert_eq!(def.constraint, None);
}

#[test]
fn danny_pink_granted_quote_uses_object_first_time_condition() {
    let parsed = parse_oracle_text(
        "Mentor\nCreatures you control have \"Whenever one or more counters are put on this creature for the first time each turn, draw a card.\"",
        "Danny Pink",
        &[],
        &["Creature".to_string()],
        &[],
    );
    let grant = parsed
        .statics
        .iter()
        .flat_map(|s| s.modifications.iter())
        .find_map(|m| match m {
            ContinuousModification::GrantTrigger { trigger } => Some(trigger.as_ref()),
            _ => None,
        })
        .expect("Danny Pink must grant a trigger");
    assert_eq!(grant.mode, TriggerMode::CounterAdded);
    assert_eq!(grant.valid_card, Some(TargetFilter::SelfRef));
    assert_eq!(
        grant.condition,
        Some(TriggerCondition::FirstTimeObjectCountersAddedThisTurn),
    );
    assert_eq!(grant.constraint, None);
}

#[test]
fn non_self_counter_first_time_stays_once_per_turn() {
    let parsed = parse_oracle_text(
        "Whenever one or more counters are put on a creature you control for the first time each turn, draw a card.",
        "Synthetic Counter Watcher",
        &[],
        &["Enchantment".to_string()],
        &[],
    );
    let def = parsed
        .triggers
        .first()
        .expect("synthetic counter watcher trigger");
    assert_eq!(def.mode, TriggerMode::CounterAdded);
    assert_ne!(def.valid_card, Some(TargetFilter::SelfRef));
    assert_eq!(def.constraint, Some(TriggerConstraint::OncePerTurn));
    assert_ne!(
        def.condition,
        Some(TriggerCondition::FirstTimeObjectCountersAddedThisTurn),
    );
}
