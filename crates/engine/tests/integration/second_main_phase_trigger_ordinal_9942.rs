//! CR 505.1b: "second main phase" counts the main phases of the current turn,
//! so a trigger printed "at the beginning of your second main phase" fires
//! only at the turn's second main phase, not at an added third one. A trigger
//! printed "postcombat main phase" fires at every postcombat main phase
//! (CR 505.1a), added ones included. Issue #9942 (Cautious Survivor with an
//! additional-main-phase effect).

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;

const SECOND_MAIN: &str = "At the beginning of your second main phase, you gain 2 life.";
const POSTCOMBAT_MAIN: &str = "At the beginning of your postcombat main phase, you gain 1 life.";
const ADD_PHASES: &str = "After the second main phase this turn, there's an additional combat phase followed by an additional main phase.";

/// Passes priority (declaring no attackers or blockers, keeping trigger
/// order as offered) until `done` holds.
fn drive_until(runner: &mut GameRunner, done: impl Fn(&GameRunner) -> bool) {
    for _ in 0..300 {
        if done(runner) {
            return;
        }
        let action = match &runner.state().waiting_for {
            WaitingFor::DeclareAttackers { .. } => GameAction::DeclareAttackers {
                attacks: vec![],
                bands: vec![],
            },
            WaitingFor::DeclareBlockers { .. } => GameAction::DeclareBlockers {
                assignments: vec![],
            },
            WaitingFor::OrderTriggers { triggers, .. } => GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            },
            _ => GameAction::PassPriority,
        };
        runner.act(action).expect("the turn advances");
    }
    panic!("the stop condition was never reached");
}

fn main_phases_begun(runner: &GameRunner) -> u32 {
    let tally = &runner.state().steps_started_this_turn;
    tally.count(Phase::PreCombatMain) + tally.count(Phase::PostCombatMain)
}

/// P0's life once the main phase numbered `n` this turn has begun and its
/// beginning-of-phase triggers have resolved.
fn life_after_main_phase(runner: &mut GameRunner, n: u32) -> i32 {
    drive_until(runner, |r| {
        main_phases_begun(r) == n
            && r.state().stack.is_empty()
            && matches!(r.state().waiting_for, WaitingFor::Priority { player } if player == P0)
    });
    runner.life(P0)
}

fn board() -> GameRunner {
    let mut scenario = GameScenario::new();
    // Begin in the upkeep so every main phase of the turn is recorded as it
    // begins; the library keeps the draw step from decking P0.
    scenario.at_phase(Phase::Upkeep);
    scenario.with_library_top(P0, &["Filler One", "Filler Two"]);
    scenario.add_creature_from_oracle(P0, "Second Main Probe", 1, 1, SECOND_MAIN);
    scenario.add_creature_from_oracle(P0, "Postcombat Probe", 1, 1, POSTCOMBAT_MAIN);
    let adder = scenario
        .add_spell_to_hand_from_oracle(P0, "Phase Adder", false, ADD_PHASES)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let mut runner = scenario.build();
    drive_until(&mut runner, |r| {
        r.state().phase == Phase::PreCombatMain
            && matches!(r.state().waiting_for, WaitingFor::Priority { player } if player == P0)
    });
    runner.cast(adder).resolve();
    runner.advance_until_stack_empty();
    runner
}

#[test]
fn second_main_phase_trigger_fires_once_and_postcombat_trigger_fires_in_each() {
    let mut runner = board();
    let start = runner.life(P0);

    // Second main phase (the natural postcombat main): both triggers fire.
    let after_second = life_after_main_phase(&mut runner, 2);
    assert_eq!(
        after_second - start,
        3,
        "second-main (+2) and postcombat-main (+1) triggers both fire"
    );

    // Third main phase (added, also postcombat): only the postcombat trigger.
    let after_third = life_after_main_phase(&mut runner, 3);
    assert_eq!(runner.state().phase, Phase::PostCombatMain);
    assert_eq!(
        after_third - after_second,
        1,
        "CR 505.1b: the added third main phase is not the second main phase"
    );
}
