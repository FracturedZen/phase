//! Gate A — Basalt Golem (#10347): delayed end-of-combat sacrifice binds the
//! blocker (`ParentTarget`), not the source (`SelfRef`).
//!
//! CR 608.2c + CR 701.21a: "Whenever this creature becomes blocked by a
//! creature, that creature's controller sacrifices it at end of combat."

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    AbilityCondition, DelayedTriggerCondition, Effect, QuantityExpr, TargetFilter,
};
use engine::types::phase::Phase;
use engine::types::triggers::TriggerMode;

const BASALT_GOLEM: &str = "This creature can't be blocked by artifact creatures.\n\
Whenever this creature becomes blocked by a creature, that creature's controller \
sacrifices it at end of combat. If the player does, they create a 0/2 colorless \
Wall artifact creature token with defender.";

#[test]
fn basalt_golem_delayed_sacrifice_is_parent_target() {
    let card = parse_oracle_text(
        BASALT_GOLEM,
        "Basalt Golem",
        &[],
        &["Artifact".into(), "Creature".into()],
        &["Golem".into()],
    );
    let trigger = card
        .triggers
        .iter()
        .find(|t| t.mode == TriggerMode::BecomesBlocked)
        .expect("BecomesBlocked trigger");
    assert!(
        trigger.valid_target.is_some(),
        "per-blocker form (CR 509.3d) must carry valid_target"
    );
    let execute = trigger.execute.as_deref().expect("execute");
    let Effect::CreateDelayedTrigger {
        condition, effect, ..
    } = execute.effect.as_ref()
    else {
        panic!("expected CreateDelayedTrigger, got {:?}", execute.effect);
    };
    assert!(
        matches!(
            condition,
            DelayedTriggerCondition::AtNextPhase {
                phase: Phase::EndCombat
            }
        ),
        "expected AtNextPhase(EndCombat), got {condition:?}"
    );
    match effect.effect.as_ref() {
        Effect::Sacrifice { target, count, .. } => {
            assert_eq!(
                *target,
                TargetFilter::ParentTarget,
                "blocker 'it' must be ParentTarget, not SelfRef"
            );
            assert_eq!(*count, QuantityExpr::Fixed { value: 1 });
        }
        other => panic!("expected Sacrifice(ParentTarget), got {other:?}"),
    }
    let wall = effect.sub_ability.as_deref().expect("Wall token sub");
    assert_eq!(wall.condition, Some(AbilityCondition::effect_performed()));
    assert!(matches!(wall.effect.as_ref(), Effect::Token { .. }));
}
