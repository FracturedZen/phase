//! Gate A — Goblin Archaeologist (#10346): one coin flip owns both the compound
//! win outcome (destroy + untap) and the lose sacrifice.
//!
//! CR 705.2 + CR 608.2c: "If you win the flip, destroy target artifact and untap
//! this creature. If you lose the flip, sacrifice this creature." — both
//! conditional instructions refer to that same flip; the untap is not
//! unconditional and the sacrifice is not a second FlipCoins.

use engine::parser::oracle::parse_oracle_text;
use engine::parser::oracle_effect::parse_effect_chain;
use engine::types::ability::{AbilityKind, Effect, TargetFilter, TapStateChange};

const GOBLIN_ARCHAEOLOGIST: &str = "{R}, {T}: Flip a coin. If you win the flip, \
destroy target artifact and untap this creature. If you lose the flip, sacrifice \
this creature.";

#[test]
fn goblin_archaeologist_one_flip_owns_win_compound_and_lose() {
    let card = parse_oracle_text(
        GOBLIN_ARCHAEOLOGIST,
        "Goblin Archaeologist",
        &[],
        &["Creature".into()],
        &["Goblin".into(), "Artificer".into()],
    );
    let activated = card
        .abilities
        .iter()
        .find(|a| a.cost.is_some())
        .expect("activated ability");
    let Effect::FlipCoins {
        flips,
        win_effect: Some(win),
        lose_effect: Some(lose),
        ..
    } = &*activated.effect
    else {
        panic!(
            "expected one FlipCoins with both branches, got {:?}",
            activated.effect
        );
    };
    assert!(
        flips.is_single_flip(),
        "exactly one logical flip, got {flips:?}"
    );
    assert!(
        matches!(&*win.effect, Effect::Destroy { .. }),
        "win head destroys the artifact, got {:?}",
        win.effect
    );
    let untap = win
        .sub_ability
        .as_deref()
        .expect("untap must live under win_effect");
    assert!(
        matches!(
            &*untap.effect,
            Effect::SetTapState {
                state: TapStateChange::Untap,
                ..
            }
        ),
        "expected win-body Untap, got {:?}",
        untap.effect
    );
    assert!(
        matches!(
            &*lose.effect,
            Effect::Sacrifice {
                target: TargetFilter::SelfRef,
                ..
            }
        ),
        "lose arm sacrifices SelfRef on the same flip, got {:?}",
        lose.effect
    );
    assert!(
        activated.sub_ability.is_none(),
        "no unconditional Untap / second flip sibling"
    );
}

/// Effect-chain path: same compound win folds through consolidate without a
/// second FlipCoins.
#[test]
fn goblin_archaeologist_effect_chain_compound_win() {
    let text = "Flip a coin. If you win the flip, destroy target artifact and untap this creature. \
        If you lose the flip, sacrifice this creature.";
    let def = parse_effect_chain(text, AbilityKind::Spell);
    let Effect::FlipCoins {
        flips,
        win_effect: Some(win),
        lose_effect: Some(lose),
        ..
    } = &*def.effect
    else {
        panic!("expected one FlipCoins with both branches, got {:?}", def.effect);
    };
    assert!(flips.is_single_flip());
    assert!(matches!(&*win.effect, Effect::Destroy { .. }));
    assert!(
        win.sub_ability.as_deref().is_some_and(|s| {
            matches!(
                &*s.effect,
                Effect::SetTapState {
                    state: TapStateChange::Untap,
                    ..
                }
            )
        }),
        "untap under win_effect, got {:?}",
        win.sub_ability
    );
    assert!(matches!(
        &*lose.effect,
        Effect::Sacrifice {
            target: TargetFilter::SelfRef,
            ..
        }
    ));
    assert!(def.sub_ability.is_none());
}

/// Reach guard: bare single flip still does not absorb a following non-branch
/// sentence into win_effect.
#[test]
fn single_flip_sibling_sentence_stays_outside_win_effect() {
    let def = parse_effect_chain("Flip a coin. Draw a card.", AbilityKind::Spell);
    let Effect::FlipCoins {
        flips,
        win_effect,
        lose_effect,
        ..
    } = &*def.effect
    else {
        panic!("expected FlipCoins, got {:?}", def.effect);
    };
    assert!(flips.is_single_flip());
    assert!(win_effect.is_none() && lose_effect.is_none());
    let sub = def.sub_ability.as_deref().expect("Draw stays sub_ability");
    assert!(matches!(&*sub.effect, Effect::Draw { .. }));
}
