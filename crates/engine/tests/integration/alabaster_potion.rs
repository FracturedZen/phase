//! Integration tests for Alabaster Potion (`{X}{W}{W}`):
//! "Choose one —
//! • You gain X life.
//! • Prevent the next X damage that would be dealt to any target this turn."
//!
//! CR 107.3: The value of variable X is chosen when the spell is cast.
//! CR 615.7: "Prevent the next X damage" sets up a depletion shield absorbing up to X damage.
//! CR 615.11: The shield lasts until end of turn or until depleted.
//! CR 608.2b: When resolving a modal spell, the chosen mode's instructions are followed.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    AbilityDefinition, Effect, PreventionAmount, QuantityExpr, QuantityRef, TargetFilter,
};
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;

const ALABASTER_POTION: &str = "Choose one —\n• You gain X life.\n• Prevent the next X damage that would be dealt to any target this turn.";
const SHOCK: &str = "Shock deals 2 damage to any target.";
const LIGHTNING_BOLT: &str = "Lightning Bolt deals 3 damage to any target.";

fn alabaster_mana_cost() -> ManaCost {
    ManaCost::Cost {
        shards: vec![ManaCostShard::X, ManaCostShard::White, ManaCostShard::White],
        generic: 0,
    }
}

fn white_mana(count: usize) -> Vec<ManaUnit> {
    vec![
        ManaUnit::new(
            ManaType::White,
            engine::types::identifiers::ObjectId(0),
            false,
            vec![],
        );
        count
    ]
}

/// Paid cast with X > 1 (X=3) and multiple damage events consuming the same depletion shield:
/// - Shock 1 (2 damage): 2 damage prevented, 1 shield capacity remaining. P1 takes 0 damage.
/// - Shock 2 (2 damage): 1 damage prevented, shield depleted. P1 takes 1 damage.
/// - Shock 3 (2 damage): shield is gone. P1 takes 2 damage.
/// Total damage prevented = 3 (CR 615.7).
#[test]
fn alabaster_potion_prevent_multiple_damage_events_depletion() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, white_mana(10));

    let potion = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Alabaster Potion", true, ALABASTER_POTION);
        b.with_mana_cost(alabaster_mana_cost());
        b.id()
    };
    let shock1 = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, SHOCK)
        .id();
    let shock2 = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, SHOCK)
        .id();
    let shock3 = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, SHOCK)
        .id();

    let mut runner = scenario.build();

    // Cast Alabaster Potion: mode 1 (prevention mode), X = 3, targeting P1.
    runner
        .cast(potion)
        .modes(&[1])
        .x(3)
        .target_player(P1)
        .resolve();

    // Damage event 1: Shock deals 2 damage to P1. Shield absorbs 2 damage (1 remaining).
    let outcome1 = runner.cast(shock1).target_player(P1).resolve();
    assert_eq!(
        outcome1.life_delta(P1),
        0,
        "first 2 damage must be fully absorbed by the X=3 shield"
    );

    // Damage event 2: Shock deals 2 damage to P1. Shield absorbs 1 damage and is consumed; P1 takes 1.
    let outcome2 = runner.cast(shock2).target_player(P1).resolve();
    assert_eq!(
        outcome2.life_delta(P1),
        -1,
        "second 2 damage must have 1 prevented and 1 dealt as shield depletes"
    );

    // Damage event 3: Shock deals 2 damage to P1. Shield is depleted; P1 takes full 2 damage.
    let outcome3 = runner.cast(shock3).target_player(P1).resolve();
    assert_eq!(
        outcome3.life_delta(P1),
        -2,
        "third shock must deal full 2 damage after shield is depleted"
    );
}

/// Paid cast with X = 0: shield has capacity 0 and absorbs no damage.
#[test]
fn alabaster_potion_prevent_x_zero() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, white_mana(10));

    let potion = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Alabaster Potion", true, ALABASTER_POTION);
        b.with_mana_cost(alabaster_mana_cost());
        b.id()
    };
    let bolt = scenario
        .add_spell_to_hand_from_oracle(P0, "Lightning Bolt", true, LIGHTNING_BOLT)
        .id();

    let mut runner = scenario.build();

    // Cast Alabaster Potion: mode 1 (prevention), X = 0, targeting P1.
    runner
        .cast(potion)
        .modes(&[1])
        .x(0)
        .target_player(P1)
        .resolve();

    // Lightning Bolt deals 3 damage to P1. X=0 shield prevents 0 damage; full 3 damage is dealt.
    let outcome = runner.cast(bolt).target_player(P1).resolve();
    assert_eq!(
        outcome.life_delta(P1),
        -3,
        "X=0 prevention shield must prevent 0 damage"
    );
}

/// Prevention mode targeting a creature permanent:
/// With X = 2, creature takes 3 damage: 2 prevented, 1 marked.
#[test]
fn alabaster_potion_prevent_target_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, white_mana(10));

    let dummy = scenario.add_creature(P1, "Target Dummy", 0, 5).id();
    let potion = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Alabaster Potion", true, ALABASTER_POTION);
        b.with_mana_cost(alabaster_mana_cost());
        b.id()
    };
    let bolt = scenario
        .add_spell_to_hand_from_oracle(P0, "Lightning Bolt", true, LIGHTNING_BOLT)
        .id();

    let mut runner = scenario.build();

    // Cast Alabaster Potion: mode 1, X = 2, targeting the dummy creature.
    runner
        .cast(potion)
        .modes(&[1])
        .x(2)
        .target_object(dummy)
        .resolve();

    // Lightning Bolt deals 3 damage to dummy. 2 damage is prevented, 1 damage is marked.
    let outcome = runner.cast(bolt).target_object(dummy).resolve();
    assert_eq!(
        outcome.damage_marked(dummy),
        1,
        "2 damage must be prevented on creature, marking 1 damage"
    );
}

/// Mode 0: "You gain X life." With X = 4, P0 gains 4 life.
#[test]
fn alabaster_potion_gain_life_mode() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, white_mana(10));

    let potion = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Alabaster Potion", true, ALABASTER_POTION);
        b.with_mana_cost(alabaster_mana_cost());
        b.id()
    };

    let mut runner = scenario.build();

    // Cast Alabaster Potion: mode 0 (gain life), X = 4.
    let outcome = runner.cast(potion).modes(&[0]).x(4).resolve();
    assert_eq!(outcome.life_delta(P0), 4, "player must gain X=4 life");
}

/// Regression coverage for normal native loaders:
/// 1. MTGJSON live parsing via `parse_oracle_text` correctly retains `Variable("X")` on mode 1.
/// 2. Export JSON loader via serde serialization and deserialization retains `amount_dynamic`.
#[test]
fn alabaster_potion_native_loaders_and_serde_roundtrip() {
    let parsed = parse_oracle_text(
        ALABASTER_POTION,
        "Alabaster Potion",
        &[],
        &["Instant".to_string()],
        &[],
    );
    assert_eq!(parsed.abilities.len(), 2, "must parse two modal abilities");

    // Check MTGJSON parse result
    let prev_ability = &parsed.abilities[1];
    match prev_ability.effect.as_ref() {
        Effect::PreventDamage {
            amount,
            amount_dynamic,
            target,
            ..
        } => {
            assert_eq!(*amount, PreventionAmount::Next(1));
            assert_eq!(
                amount_dynamic.as_ref(),
                Some(&QuantityExpr::Ref {
                    qty: QuantityRef::Variable {
                        name: "X".to_string(),
                    },
                }),
                "native parse must populate amount_dynamic with Variable(X)"
            );
            assert_eq!(*target, TargetFilter::Any);
        }
        other => panic!("expected PreventDamage effect, got {other:?}"),
    }

    // Check export JSON loader (serde roundtrip of AbilityDefinition)
    let json = serde_json::to_string(&parsed.abilities).expect("must serialize abilities to JSON");
    let deserialized: Vec<AbilityDefinition> =
        serde_json::from_str(&json).expect("must deserialize abilities from JSON");

    assert_eq!(deserialized.len(), 2);
    match deserialized[1].effect.as_ref() {
        Effect::PreventDamage {
            amount,
            amount_dynamic,
            target,
            ..
        } => {
            assert_eq!(*amount, PreventionAmount::Next(1));
            assert_eq!(
                amount_dynamic.as_ref(),
                Some(&QuantityExpr::Ref {
                    qty: QuantityRef::Variable {
                        name: "X".to_string(),
                    },
                }),
                "deserialized export entry must retain amount_dynamic with Variable(X)"
            );
            assert_eq!(*target, TargetFilter::Any);
        }
        other => panic!("expected PreventDamage effect after serde roundtrip, got {other:?}"),
    }
}
