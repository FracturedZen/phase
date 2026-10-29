//! Vanish into Memory (issue #9842): "If you do, discard cards equal to that
//! creature's toughness." The delayed trigger's "that creature" is the card it
//! just returned to the battlefield (CR 608.2c, CR 400.7), so the discard
//! count is the returned creature's toughness, read as the instruction is
//! applied (CR 608.2h). Before the fix the player-scoped discard inherited
//! the returned card's stale exile-time pin as chain context and was skipped
//! entirely, as if its own referent had left (CR 400.7).

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const VANISH_INTO_MEMORY: &str = "Exile target creature. You draw cards equal to that creature's power. At the beginning of your next upkeep, return that card to the battlefield under its owner's control. If you do, discard cards equal to that creature's toughness.";

/// Passes priority (no attackers, no blockers, offered trigger order) until
/// P0 is asked to discard, recording nothing else. Returns the discard count,
/// or `None` if P0's next turn reaches its draw step without a discard.
fn drive_to_discard(runner: &mut GameRunner) -> Option<usize> {
    let mut left_first_turn = false;
    for _ in 0..600 {
        let state = runner.state();
        if state.active_player == P1 {
            left_first_turn = true;
        }
        if left_first_turn && state.active_player == P0 && state.phase == Phase::Draw {
            return None;
        }
        let action = match &state.waiting_for {
            WaitingFor::DiscardChoice { player, count, .. } => {
                assert_eq!(*player, P0, "the spell's controller discards");
                return Some(*count);
            }
            WaitingFor::OrderTriggers { triggers, .. } => GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            },
            WaitingFor::DeclareAttackers { .. } => GameAction::DeclareAttackers {
                attacks: vec![],
                bands: vec![],
            },
            WaitingFor::DeclareBlockers { .. } => GameAction::DeclareBlockers {
                assignments: vec![],
            },
            WaitingFor::Priority { .. } => GameAction::PassPriority,
            other => panic!("unexpected prompt: {other:?}"),
        };
        runner.act(action).expect("the game advances");
    }
    panic!("P0's next turn was never reached");
}

fn cast_vanish(power: i32, toughness: i32) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    for i in 0..8 {
        scenario.add_card_to_library_top(P0, &format!("P0 Library {i}"));
        scenario.add_card_to_library_top(P1, &format!("P1 Library {i}"));
    }
    for i in 0..5 {
        scenario.add_creature_to_hand(P0, &format!("P0 Hand {i}"), 1, 1);
    }
    let creature = scenario
        .add_creature(P0, "Vanishing Beast", power, toughness)
        .id();
    let vanish = scenario
        .add_spell_to_hand_from_oracle(P0, "Vanish into Memory", false, VANISH_INTO_MEMORY)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let mut runner = scenario.build();
    runner.cast(vanish).target_object(creature).resolve();
    runner.advance_until_stack_empty();
    (runner, creature)
}

#[test]
fn returned_creature_toughness_sets_the_discard_count() {
    let (mut runner, creature) = cast_vanish(2, 3);
    assert_eq!(runner.state().objects[&creature].zone, Zone::Exile);

    let count = drive_to_discard(&mut runner);
    assert_eq!(
        count,
        Some(3),
        "CR 608.2c: discard equal to the returned creature's toughness (3)"
    );
}

#[test]
fn draw_still_follows_the_exiled_creature_power() {
    let hand_before = {
        let (runner, _) = cast_vanish(1, 1);
        runner.state().players[0].hand.len()
    };
    let (runner, _) = cast_vanish(3, 1);
    assert_eq!(
        runner.state().players[0].hand.len(),
        hand_before + 2,
        "power 3 draws two more cards than power 1"
    );
}
