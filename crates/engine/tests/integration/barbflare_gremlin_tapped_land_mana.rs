//! Barbflare Gremlin (#10275): "Whenever a player taps a land for mana, if this
//! creature is tapped, that player adds one mana of any type that land
//! produced. Then that land deals 1 damage to that player."
//!
//! CR 605.1b: a triggered mana ability that triggers from a mana ability and
//! could add mana is a mana ability, so it resolves immediately (CR 605.4a).
//! CR 603.4: "if this creature is tapped" is an intervening-if condition,
//! checked when the land is tapped and again on resolution.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaColor, ManaType};
use engine::types::phase::Phase;

const BARBFLARE_GREMLIN: &str = "First strike, haste\nWhenever a player taps a land for mana, if this creature is tapped, that player adds one mana of any type that land produced. Then that land deals 1 damage to that player.";

fn gremlin_board(gremlin_tapped: bool) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let gremlin = scenario
        .add_creature_from_oracle(P0, "Barbflare Gremlin", 3, 2, BARBFLARE_GREMLIN)
        .id();
    let forest = scenario.add_basic_land(P0, ManaColor::Green);
    let mut runner = scenario.build();
    runner
        .state_mut()
        .objects
        .get_mut(&gremlin)
        .expect("the Gremlin is on the battlefield")
        .tapped = gremlin_tapped;
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

fn green(runner: &GameRunner) -> usize {
    runner.state().players[P0.0 as usize]
        .mana_pool
        .count_color(ManaType::Green)
}

/// CR 605.1b + CR 603.4: with the Gremlin tapped, tapping a Forest for mana
/// adds one more green mana, then the Forest deals 1 damage to its controller.
#[test]
fn tapped_gremlin_adds_mana_and_the_land_deals_one_damage() {
    let (mut runner, forest) = gremlin_board(true);
    tap_for_mana(&mut runner, forest);
    assert!(
        runner.state().stack.is_empty(),
        "CR 605.4a: the triggered mana ability resolves without the stack"
    );
    assert_eq!(green(&runner), 2, "the Forest's G plus the Gremlin's G");
    assert_eq!(runner.life(P0), 19, "then the Forest deals 1 damage");
}

/// CR 603.4: an untapped Gremlin fails its intervening-if, so nothing extra
/// happens.
#[test]
fn untapped_gremlin_adds_nothing() {
    let (mut runner, forest) = gremlin_board(false);
    tap_for_mana(&mut runner, forest);
    assert_eq!(green(&runner), 1);
    assert_eq!(runner.life(P0), 20);
}
