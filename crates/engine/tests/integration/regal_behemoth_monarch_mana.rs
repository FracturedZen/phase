//! Regal Behemoth (#8152): "Whenever you tap a land for mana while you're the
//! monarch, add an additional one mana of any color."
//!
//! CR 605.1b: the trigger is a triggered mana ability and resolves immediately
//! (CR 605.4a). CR 603.4 + CR 724.1: "while you're the monarch" gates the
//! trigger, so a player who isn't the monarch gets nothing extra.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaColor;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const REGAL_BEHEMOTH: &str = "Trample\nWhen this creature enters, you become the monarch.\nWhenever you tap a land for mana while you're the monarch, add an additional one mana of any color.";

fn behemoth_board(monarch: Option<PlayerId>) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, "Regal Behemoth", 5, 5, REGAL_BEHEMOTH);
    let forest = scenario.add_basic_land(P0, ManaColor::Green);
    let mut runner = scenario.build();
    runner.state_mut().monarch = monarch;
    (runner, forest)
}

fn tap_for_mana(runner: &mut GameRunner, land: ObjectId) {
    let (_, _, grouped) = engine::ai_support::legal_actions_full(runner.state());
    let selection = grouped
        .get(&land)
        .into_iter()
        .flatten()
        .find_map(|action| match action {
            GameAction::TapLandForMana { selection } => Some(selection.clone()),
            _ => None,
        })
        .expect("the Forest offers a tap-for-mana action");
    runner
        .act(GameAction::TapLandForMana { selection })
        .expect("tapping the Forest for mana is legal");
}

fn pool_total(runner: &GameRunner) -> usize {
    runner.state().players[P0.0 as usize].mana_pool.total()
}

/// CR 605.4a: as the monarch, tapping a Forest adds its G plus one extra mana,
/// without using the stack.
#[test]
fn monarch_gets_the_additional_mana() {
    let (mut runner, forest) = behemoth_board(Some(P0));
    tap_for_mana(&mut runner, forest);
    assert!(runner.state().stack.is_empty());
    assert_eq!(pool_total(&runner), 2, "the Forest's G plus the extra mana");
}

/// CR 603.4: with no monarch, the condition is false and no extra mana is added.
#[test]
fn no_monarch_gets_no_additional_mana() {
    let (mut runner, forest) = behemoth_board(None);
    tap_for_mana(&mut runner, forest);
    assert!(runner.state().stack.is_empty());
    assert_eq!(pool_total(&runner), 1, "only the Forest's G");
}

/// CR 603.4: an opponent being the monarch doesn't satisfy "you're the monarch".
#[test]
fn opponent_monarch_gets_no_additional_mana() {
    let (mut runner, forest) = behemoth_board(Some(P1));
    tap_for_mana(&mut runner, forest);
    assert!(runner.state().stack.is_empty());
    assert_eq!(pool_total(&runner), 1, "only the Forest's G");
}
