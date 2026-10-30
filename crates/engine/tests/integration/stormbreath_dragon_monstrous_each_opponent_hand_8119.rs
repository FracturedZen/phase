//! Issue #8119 — Stormbreath Dragon: becoming monstrous dealt no damage.
//!
//! "When this creature becomes monstrous, it deals damage to each opponent
//! equal to the number of cards in that player's hand."
//!
//! CR 701.37b: the trigger fires when the Monstrosity ability makes the
//! creature monstrous. CR 120.3 + CR 608.2c: "that player" is each opponent in
//! turn, so each opponent takes damage equal to the cards in their OWN hand,
//! and no player target is ever announced. The pre-fix parse lowered the
//! amount to a `TargetZoneCardCount`, which surfaced an undeclared
//! player-target slot so the trigger stalled at target selection and never
//! dealt its per-opponent damage.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::ability::Effect;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::player::PlayerId;

const P2: PlayerId = PlayerId(2);
use engine::types::phase::Phase;

/// Verbatim Oracle text (Scryfall) minus reminder text.
const STORMBREATH_DRAGON: &str = "Flying, haste, protection from white\n\
{5}{R}{R}: Monstrosity 3. (If this creature isn't monstrous, put three +1/+1 counters on it and it becomes monstrous.)\n\
When this creature becomes monstrous, it deals damage to each opponent equal to the number of cards in that player's hand.";

#[test]
fn stormbreath_monstrous_damages_each_opponent_by_own_hand_size_8119() {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let dragon = scenario
        .add_creature_from_oracle(P0, "Stormbreath Dragon", 4, 4, STORMBREATH_DRAGON)
        .id();
    // Opponent hand sizes differ so a shared/first-target count is caught.
    for i in 0..3 {
        scenario.add_card_to_hand(P1, &format!("P1 Card {i}"));
    }
    scenario.add_card_to_hand(P2, "P2 Card");
    // The controller's own hand must not be counted or damaged.
    for i in 0..2 {
        scenario.add_card_to_hand(P0, &format!("P0 Card {i}"));
    }
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Red, ObjectId(9999), false, vec![]); 7],
    );
    let mut runner = scenario.build();

    let monstrosity_idx = runner.state().objects[&dragon]
        .abilities
        .iter()
        .position(|a| matches!(*a.effect, Effect::Monstrosity { .. }))
        .expect("Stormbreath must have a Monstrosity ability");

    let p0_before = runner.state().players[0].life;
    let p1_before = runner.state().players[1].life;
    let p2_before = runner.state().players[2].life;

    // No player target is declared: the printed ability and trigger name none.
    runner.activate(dragon, monstrosity_idx).resolve();
    runner.advance_until_stack_empty();

    let state = runner.state();
    let obj = &state.objects[&dragon];
    assert!(obj.monstrous, "dragon must be monstrous");
    assert_eq!(
        state.players[1].life,
        p1_before - 3,
        "P1 (3 cards in hand) must take 3"
    );
    assert_eq!(
        state.players[2].life,
        p2_before - 1,
        "P2 (1 card in hand) must take 1"
    );
    assert_eq!(
        state.players[0].life, p0_before,
        "the controller is not an opponent and must take no damage"
    );
}
