//! Issue #9607 — Nettling Nuisance singular "The token is goaded …" after a
//! with-"…" grant must chain as a permanent goad on LastCreated.

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{ContinuousModification, Duration, Effect, TargetFilter};
use engine::types::statics::StaticMode;

const NETTLING_NUISANCE: &str = "Whenever one or more Faeries you control deal combat damage to a player, that player creates a 4/2 red Pirate creature token with \"This token can't block.\" The token is goaded for the rest of the game.";

#[test]
fn nettling_nuisance_goads_created_token() {
    let parsed = parse_oracle_text(
        NETTLING_NUISANCE,
        "Nettling Nuisance",
        &[],
        &["Creature".to_string()],
        &["Faerie".to_string()],
    );
    let execute = parsed.triggers[0]
        .execute
        .as_deref()
        .expect("trigger body");
    assert!(
        matches!(*execute.effect, Effect::Token { .. }),
        "expected Token head, got {:?}",
        execute.effect
    );
    let sub = execute
        .sub_ability
        .as_deref()
        .expect("goad sub_ability must be present (#9607)");
    match &*sub.effect {
        Effect::GenericEffect {
            static_abilities,
            duration,
            target,
            end_cost: _,
        } => {
            assert_eq!(*target, Some(TargetFilter::LastCreated));
            assert_eq!(*duration, Some(Duration::Permanent));
            assert!(
                static_abilities[0].modifications.iter().any(|m| matches!(
                    m,
                    ContinuousModification::AddStaticMode {
                        mode: StaticMode::Goaded
                    }
                )),
                "expected Goaded static mode, got {static_abilities:?}"
            );
        }
        other => panic!("expected GenericEffect goad sub, got {other:?}"),
    }
}
