//! Issue #9946 — Mana Drain's mana must arrive at the controller's next main
//! phase, which is the same turn's postcombat main phase when Mana Drain
//! resolves during the precombat main phase.
//!
//! CR 505.1: "main phase" names both the precombat and the postcombat main
//! phase, so "at the beginning of your next main phase" (CR 603.7a delayed
//! trigger) fires at whichever of the controller's main phases begins first.
//!
//! https://github.com/phase-rs/phase/issues/9946

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

// Verbatim Oracle text (Scryfall).
const MANA_DRAIN: &str = "Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell's mana value.";

fn colorless(n: usize) -> Vec<ManaUnit> {
    (0..n)
        .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
        .collect()
}

/// Both players pass priority, step by step, until `phase` begins — the real
/// turn-structure path, so every `PhaseChanged` reaches the delayed-trigger
/// matcher (CR 500.2, CR 603.7).
fn pass_priority_until(runner: &mut GameRunner, phase: Phase) {
    for _ in 0..24 {
        if runner.state().phase == phase {
            return;
        }
        runner.act(GameAction::PassPriority).expect("pass priority");
    }
    panic!("never reached {phase:?}");
}

/// P0, in `phase` of its own turn, casts a mana-value-4 creature and counters
/// it with its own Mana Drain (the issue's measured line).
fn drain_own_four_drop(phase: Phase) -> GameRunner {
    let mut scenario = GameScenario::new();
    scenario.at_phase(phase);
    scenario.with_mana_pool(P0, colorless(4));
    let giant = scenario
        .add_creature_to_hand(P0, "Hill Giant", 3, 3)
        .with_mana_cost(ManaCost::generic(4))
        .id();
    let drain = scenario
        .add_spell_to_hand_from_oracle(P0, "Mana Drain", true, MANA_DRAIN)
        .id();
    let mut runner = scenario.build();

    runner.cast(giant).commit();
    runner.cast(drain).target_object(giant).resolve();
    assert_eq!(
        runner.state().objects[&giant].zone,
        Zone::Graveyard,
        "reach-guard: Mana Drain counters Hill Giant"
    );
    assert_eq!(
        runner.state().delayed_triggers.len(),
        1,
        "reach-guard: Mana Drain creates its delayed mana trigger"
    );
    assert_eq!(
        runner.state().players[0]
            .mana_pool
            .count_color(ManaType::Colorless),
        0
    );
    runner
}

#[test]
fn mana_drain_cast_precombat_adds_mana_in_the_same_turns_postcombat_main() {
    let mut runner = drain_own_four_drop(Phase::PreCombatMain);
    let turn = runner.state().turn_number;

    pass_priority_until(&mut runner, Phase::PostCombatMain);
    runner.advance_until_stack_empty();

    assert_eq!(runner.state().turn_number, turn, "still the same turn");
    assert_eq!(runner.state().phase, Phase::PostCombatMain);
    assert_eq!(
        runner.state().players[0]
            .mana_pool
            .count_color(ManaType::Colorless),
        4,
        "CR 505.1: the postcombat main phase is P0's next main phase, so the \
         four {{C}} arrive now"
    );
    assert!(
        runner.state().delayed_triggers.is_empty(),
        "the one-shot delayed trigger is spent (CR 603.7c)"
    );
}

/// Control: cast during the postcombat main phase, the next main phase is a
/// later turn's — nothing fires during the rest of this turn.
#[test]
fn mana_drain_cast_postcombat_waits_past_this_turns_end_step() {
    let mut runner = drain_own_four_drop(Phase::PostCombatMain);
    let turn = runner.state().turn_number;

    pass_priority_until(&mut runner, Phase::End);

    assert_eq!(runner.state().turn_number, turn);
    assert_eq!(runner.state().phase, Phase::End);
    assert_eq!(
        runner.state().players[0]
            .mana_pool
            .count_color(ManaType::Colorless),
        0
    );
    assert_eq!(
        runner.state().delayed_triggers.len(),
        1,
        "the trigger waits for P0's next main phase"
    );
}
