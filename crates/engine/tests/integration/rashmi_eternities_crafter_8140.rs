//! Issue #8140 — Rashmi, Eternities Crafter:
//! "Whenever you cast your first spell each turn, reveal the top card of your library.
//! You may cast it without paying its mana cost if it's a spell with lesser mana value.
//! If you don't cast it, put it into your hand."
//!
//! Under CR 608.2h and CR 202.3:
//! - "lesser mana value" references the characteristics of the spell on the stack
//!   that caused the ability to trigger (`ObjectScope::EventSource`).
//! - A card revealed this way can only be cast if it is a spell (nonland) and its
//!   mana value is strictly less than the triggering spell's mana value (CR 601.2, CR 305.9).
//! - If the revealed card does not satisfy this condition (e.g. MV >= triggering spell,
//!   or is a land), casting is impossible and the player is not offered an invalid
//!   cast choice (CR 608.2d).
//! - "If you don't cast it, put it into your hand" puts the revealed card into hand
//!   when the cast is not performed (whether because the player declined or because
//!   casting was ineligible/impossible).

use engine::game::scenario::{GameScenario, P0};
use engine::parser::parse_oracle_text;
use engine::types::ability::{
    AbilityCondition, CastPermissionConstraint, Comparator, Effect, EffectOutcomeSignal,
    ObjectScope, QuantityExpr, QuantityRef,
};
use engine::types::actions::GameAction;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const RASHMI_ORACLE: &str = "Whenever you cast your first spell each turn, reveal the top \
                             card of your library. You may cast it without paying its \
                             mana cost if it's a spell with lesser mana value. If you \
                             don't cast it, put it into your hand.";
const DIVINATION: &str = "Draw two cards.";

fn floating_mana(n: usize, ty: ManaType) -> Vec<ManaUnit> {
    (0..n)
        .map(|_| ManaUnit::new(ty, ObjectId(0), false, vec![]))
        .collect()
}

#[test]
fn rashmi_parses_relative_mv_constraint_and_hand_fallback() {
    let parsed = parse_oracle_text(
        RASHMI_ORACLE,
        "Rashmi, Eternities Crafter",
        &[],
        &["Creature".to_string()],
        &[],
    );

    let trigger = parsed
        .triggers
        .first()
        .expect("Rashmi must have a parsed trigger");
    let head = trigger.execute.as_ref().expect("trigger execute");

    assert!(matches!(*head.effect, Effect::RevealTop { count: 1, .. }));

    let cast_sub = head.sub_ability.as_ref().expect("CastFromZone sub");
    assert!(cast_sub.optional, "cast must be optional");
    assert!(
        matches!(
            *cast_sub.effect,
            Effect::CastFromZone {
                without_paying_mana_cost: true,
                constraint: Some(CastPermissionConstraint::ManaValue {
                    comparator: Comparator::LT,
                    value: QuantityExpr::Ref {
                        qty: QuantityRef::ObjectManaValue {
                            scope: ObjectScope::EventSource,
                        },
                    },
                }),
                ..
            }
        ),
        "cast must carry strictly lesser mana value constraint relative to EventSource"
    );

    let hand_sub = cast_sub.sub_ability.as_ref().expect("hand fallback sub");
    assert!(
        matches!(
            *hand_sub.effect,
            Effect::ChangeZone {
                destination: Zone::Hand,
                ..
            }
        ),
        "hand fallback puts card into Hand"
    );
    assert!(
        matches!(
            hand_sub.condition,
            Some(AbilityCondition::Not { ref condition })
                if matches!(
                    **condition,
                    AbilityCondition::EffectOutcome {
                        signal: EffectOutcomeSignal::OptionalEffectPerformed,
                    }
                )
        ),
        "hand fallback is gated on Not(OptionalEffectPerformed)"
    );
}

#[test]
fn rashmi_casts_spell_with_strictly_lesser_mana_value_for_free() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, floating_mana(5, ManaType::Blue));
    scenario.add_creature_from_oracle(P0, "Rashmi, Eternities Crafter", 2, 3, RASHMI_ORACLE);

    let shock = scenario
        .add_spell_to_library_top(P0, "Shock", false)
        .with_mana_cost(ManaCost::generic(1))
        .id();

    let divination = scenario
        .add_spell_to_hand_from_oracle(P0, "Divination", false, DIVINATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    let mut commit = runner.cast(divination).commit();

    let mut offered = false;
    for _ in 0..30 {
        if commit.state().stack.is_empty()
            && matches!(
                commit.state().waiting_for,
                engine::types::game_state::WaitingFor::Priority { .. }
            )
        {
            break;
        }
        match &commit.state().waiting_for {
            engine::types::game_state::WaitingFor::Priority { .. } => {
                let _ = commit.act(GameAction::PassPriority);
            }
            engine::types::game_state::WaitingFor::OptionalEffectChoice { .. } => {
                offered = true;
                let _ = commit.act(GameAction::DecideOptionalEffect { accept: true });
            }
            _ => break,
        }
    }

    assert!(offered, "free cast must be offered for MV 1 < MV 3");
    assert_eq!(
        commit.state().objects.get(&shock).map(|o| o.zone),
        Some(Zone::Graveyard),
        "accepted Shock must be cast and resolve to graveyard"
    );
}

#[test]
fn rashmi_declining_puts_lesser_mana_value_spell_into_hand() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, floating_mana(5, ManaType::Blue));
    scenario.add_creature_from_oracle(P0, "Rashmi, Eternities Crafter", 2, 3, RASHMI_ORACLE);

    let shock = scenario
        .add_spell_to_library_top(P0, "Shock", false)
        .with_mana_cost(ManaCost::generic(1))
        .id();

    let divination = scenario
        .add_spell_to_hand_from_oracle(P0, "Divination", false, DIVINATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    let mut commit = runner.cast(divination).commit();

    let mut offered = false;
    for _ in 0..30 {
        if commit.state().stack.is_empty()
            && matches!(
                commit.state().waiting_for,
                engine::types::game_state::WaitingFor::Priority { .. }
            )
        {
            break;
        }
        match &commit.state().waiting_for {
            engine::types::game_state::WaitingFor::Priority { .. } => {
                let _ = commit.act(GameAction::PassPriority);
            }
            engine::types::game_state::WaitingFor::OptionalEffectChoice { .. } => {
                offered = true;
                let _ = commit.act(GameAction::DecideOptionalEffect { accept: false });
            }
            _ => break,
        }
    }

    assert!(offered, "free cast must be offered for MV 1 < MV 3");
    assert_eq!(
        commit.state().objects.get(&shock).map(|o| o.zone),
        Some(Zone::Hand),
        "declined Shock must be put into hand"
    );
}

#[test]
fn rashmi_greater_mana_value_spell_is_not_castable_and_goes_to_hand() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, floating_mana(5, ManaType::Blue));
    scenario.add_creature_from_oracle(P0, "Rashmi, Eternities Crafter", 2, 3, RASHMI_ORACLE);

    let big_spell = scenario
        .add_spell_to_library_top(P0, "Big Spell", false)
        .with_mana_cost(ManaCost::generic(4))
        .id();

    let divination = scenario
        .add_spell_to_hand_from_oracle(P0, "Divination", false, DIVINATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    let mut commit = runner.cast(divination).commit();

    let mut offered = false;
    for _ in 0..30 {
        if commit.state().stack.is_empty()
            && matches!(
                commit.state().waiting_for,
                engine::types::game_state::WaitingFor::Priority { .. }
            )
        {
            break;
        }
        match &commit.state().waiting_for {
            engine::types::game_state::WaitingFor::Priority { .. } => {
                let _ = commit.act(GameAction::PassPriority);
            }
            engine::types::game_state::WaitingFor::OptionalEffectChoice { .. } => {
                offered = true;
                let _ = commit.act(GameAction::DecideOptionalEffect { accept: true });
            }
            _ => break,
        }
    }

    assert!(!offered, "free cast must NOT be offered for MV 4 > MV 3");
    assert_eq!(
        commit.state().objects.get(&big_spell).map(|o| o.zone),
        Some(Zone::Hand),
        "MV 4 spell must go directly to hand"
    );
}

#[test]
fn rashmi_equal_mana_value_spell_is_not_castable_and_goes_to_hand() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, floating_mana(5, ManaType::Blue));
    scenario.add_creature_from_oracle(P0, "Rashmi, Eternities Crafter", 2, 3, RASHMI_ORACLE);

    let equal_spell = scenario
        .add_spell_to_library_top(P0, "Equal Spell", false)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let divination = scenario
        .add_spell_to_hand_from_oracle(P0, "Divination", false, DIVINATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    let mut commit = runner.cast(divination).commit();

    let mut offered = false;
    for _ in 0..30 {
        if commit.state().stack.is_empty()
            && matches!(
                commit.state().waiting_for,
                engine::types::game_state::WaitingFor::Priority { .. }
            )
        {
            break;
        }
        match &commit.state().waiting_for {
            engine::types::game_state::WaitingFor::Priority { .. } => {
                let _ = commit.act(GameAction::PassPriority);
            }
            engine::types::game_state::WaitingFor::OptionalEffectChoice { .. } => {
                offered = true;
                let _ = commit.act(GameAction::DecideOptionalEffect { accept: true });
            }
            _ => break,
        }
    }

    assert!(!offered, "free cast must NOT be offered for MV 3 == MV 3");
    assert_eq!(
        commit.state().objects.get(&equal_spell).map(|o| o.zone),
        Some(Zone::Hand),
        "equal MV spell must go directly to hand"
    );
}

#[test]
fn rashmi_revealed_land_cannot_be_cast_and_goes_to_hand() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, floating_mana(5, ManaType::Blue));
    scenario.add_creature_from_oracle(P0, "Rashmi, Eternities Crafter", 2, 3, RASHMI_ORACLE);

    let island = scenario.add_land_to_library_top(P0, "Island").id();

    let divination = scenario
        .add_spell_to_hand_from_oracle(P0, "Divination", false, DIVINATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();

    let mut runner = scenario.build();
    let mut commit = runner.cast(divination).commit();

    let mut offered = false;
    for _ in 0..30 {
        if commit.state().stack.is_empty()
            && matches!(
                commit.state().waiting_for,
                engine::types::game_state::WaitingFor::Priority { .. }
            )
        {
            break;
        }
        match &commit.state().waiting_for {
            engine::types::game_state::WaitingFor::Priority { .. } => {
                let _ = commit.act(GameAction::PassPriority);
            }
            engine::types::game_state::WaitingFor::OptionalEffectChoice { .. } => {
                offered = true;
                let _ = commit.act(GameAction::DecideOptionalEffect { accept: true });
            }
            _ => break,
        }
    }

    assert!(!offered, "free cast must NOT be offered for land");
    assert_eq!(
        commit.state().objects.get(&island).map(|o| o.zone),
        Some(Zone::Hand),
        "revealed land must go directly to hand"
    );
}
