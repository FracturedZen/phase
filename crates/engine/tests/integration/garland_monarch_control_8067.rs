//! Issue #8067: Garland's enters ability makes an opponent the monarch, and
//! that event must cause his other ability to take a creature that opponent
//! controls for as long as they remain the monarch.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const GARLAND: &str = "When Garland enters, target opponent becomes the monarch.\nWhenever an opponent becomes the monarch, gain control of target creature that player controls for as long as they're the monarch.\nCreatures you control but don't own get +2/+2 and can't be sacrificed.";

#[test]
fn garland_steals_a_creature_when_his_enters_ability_crowns_an_opponent() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(
        P0,
        vec![
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Blue, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Black, ObjectId(0), false, vec![]),
        ],
    );

    let garland = scenario
        .add_creature_to_hand_from_oracle(P0, "Garland, Royal Kidnapper", 3, 4, GARLAND)
        .with_subtypes(vec!["Human", "Knight"])
        .as_legendary()
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Blue, ManaCostShard::Black],
            generic: 2,
        })
        .id();
    let stolen = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();
    let bystander = scenario.add_creature(P1, "Runeclaw Bear", 2, 2).id();
    let ours = scenario.add_creature(P0, "Watchwolf", 3, 3).id();

    let mut runner = scenario.build();
    let outcome = runner
        .cast(garland)
        .target_player(P1)
        .target_object(stolen)
        .resolve();

    assert!(
        matches!(outcome.final_waiting_for(), WaitingFor::Priority { .. })
            && outcome.state().stack.is_empty(),
        "Garland, the monarch trigger, and the control trigger must finish, got {:?}",
        outcome.final_waiting_for()
    );
    outcome.assert_zone(&[garland], Zone::Battlefield);
    assert_eq!(
        outcome.state().monarch,
        Some(P1),
        "CR 725.1 reach guard: the enters ability makes the targeted opponent the monarch"
    );
    assert_eq!(
        outcome.controller(stolen),
        P0,
        "CR 603.2 + CR 725.1: an opponent becoming the monarch triggers Garland and he gains control of a creature that player controls"
    );
    assert_eq!(
        outcome.state().objects[&stolen].owner,
        P1,
        "gain control changes the controller and leaves the owner"
    );
    assert_eq!(
        outcome.controller(bystander),
        P1,
        "a creature that opponent controls which was not targeted stays theirs"
    );
    assert_eq!(
        outcome.controller(ours),
        P0,
        "a creature Garland's controller already owns is not the stolen creature"
    );
}
