//! Issue #6375 (dup #6910): Goreclaw, Terror of Qal Sisma — "Creature spells you
//! cast with power 4 or greater cost {2} less to cast." reduced EVERY spell.
//!
//! CR 601.2f: a cost reduction applies only to spells matching its filter.
//! CR 208.1/208.3: a creature spell's power is the printed number on the card.

use engine::game::scenario::{GameScenario, P0};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard};
use engine::types::phase::Phase;

const GORECLAW: &str = "Creature spells you cast with power 4 or greater cost {2} less to cast.\nWhenever Goreclaw attacks, each creature you control with power 4 or greater gets +1/+1 and gains trample until end of turn.";

fn generic(runner: &engine::game::scenario::GameRunner, id: ObjectId) -> u32 {
    match engine::game::casting::display_spell_cost(runner.state(), P0, id).expect("cost") {
        ManaCost::Cost { generic, .. } => generic,
        other => panic!("expected ManaCost::Cost, got {other:?}"),
    }
}

#[test]
fn goreclaw_reduces_only_power_four_or_greater_creature_spells() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, "Goreclaw, Terror of Qal Sisma", 4, 3, GORECLAW);
    let dreadmaw = scenario
        .add_creature_to_hand_from_oracle(P0, "Colossal Dreadmaw", 6, 6, "Trample")
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Green],
            generic: 5,
        })
        .id();
    let bears = scenario
        .add_creature_to_hand_from_oracle(P0, "Grizzly Bears", 2, 2, "")
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Green],
            generic: 1,
        })
        .id();
    let overrun = scenario
        .add_spell_to_hand_from_oracle(
            P0,
            "Overrun",
            false,
            "Creatures you control get +3/+3 and gain trample until end of turn.",
        )
        .with_mana_cost(ManaCost::Cost {
            shards: vec![
                ManaCostShard::Green,
                ManaCostShard::Green,
                ManaCostShard::Green,
            ],
            generic: 2,
        })
        .id();
    let runner = scenario.build();

    assert_eq!(
        generic(&runner, dreadmaw),
        3,
        "power-6 creature spell: {{5}} -> {{3}}"
    );
    assert_eq!(
        generic(&runner, bears),
        1,
        "power-2 creature spell is not reduced"
    );
    assert_eq!(
        generic(&runner, overrun),
        2,
        "noncreature spell is not reduced"
    );
}
