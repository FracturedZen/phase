//! Issue #9853 regression test: Rescuer Sphinx
//! "Flying\nAs this creature enters, you may return a nonland permanent you control to its owner's hand. If you do, this creature enters with a +1/+1 counter on it."
//!
//! CR 614.1c + CR 614.12a: "As this creature enters, you may <cost>. If you do, this creature enters with <counters>"
//! is an ETB replacement effect with `ReplacementMode::MayCost`.
//! - Accepting the choice pays the cost (returning a nonland permanent) and applies the counter placement.
//! - Declining the choice leaves the permanent on the battlefield and the creature enters with no counters.
//! - If the player controls no nonland permanents, the cost is unpayable and the creature enters with no counters.

use engine::game::effects::change_zone::resolve;
use engine::game::scenario::{GameScenario, P0};
use engine::types::ability::{ResolvedAbility, TargetFilter};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const RESCUER_SPHINX_ORACLE: &str =
    "Flying\nAs this creature enters, you may return a nonland permanent you control to its owner's hand. If you do, this creature enters with a +1/+1 counter on it.";

fn enter_via_change_zone(runner_state: &mut engine::types::game_state::GameState, card: ObjectId) {
    let ability = ResolvedAbility::new(
        engine::types::ability::Effect::ChangeZone {
            origin: Some(Zone::Hand),
            destination: Zone::Battlefield,
            target: TargetFilter::SpecificObject { id: card },
            owner_library: false,
            enter_transformed: false,
            enters_under: None,
            enter_tapped: engine::types::zones::EtbTapState::Unspecified,
            enters_attacking: false,
            up_to: false,
            enter_with_counters: vec![],
            conditional_enter_with_counters: vec![],
            face_down_profile: None,
            enters_modified_if: None,
        },
        vec![],
        ObjectId(9000),
        PlayerId(0),
    );
    let mut events = Vec::new();
    resolve(runner_state, &ability, &mut events)
        .expect("Rescuer Sphinx enter resolves into a MayCost pause");
}

#[test]
fn rescuer_sphinx_accept_returns_nonland_permanent_and_enters_with_counter() {
    let mut scenario = GameScenario::new();
    let sphinx = scenario
        .add_creature_to_hand(P0, "Rescuer Sphinx", 3, 2)
        .from_oracle_text(RESCUER_SPHINX_ORACLE)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let mut runner = scenario.build();

    enter_via_change_zone(runner.state_mut(), sphinx);

    // Replacement pauses for the accept/decline choice.
    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::ReplacementChoice { .. }
        ),
        "expected Rescuer Sphinx MayCost ReplacementChoice, got {:?}",
        runner.state().waiting_for
    );

    // Accept (index 0): with exactly 1 nonland permanent, the return is forced.
    runner
        .act(GameAction::ChooseReplacement { index: 0 })
        .expect("accepting the return cost should resolve");

    assert_eq!(
        runner.state().objects[&bear].zone,
        Zone::Hand,
        "accepting the cost must return the nonland permanent to its owner's hand"
    );
    assert_eq!(
        runner.state().objects[&sphinx].zone,
        Zone::Battlefield,
        "Rescuer Sphinx enters the battlefield"
    );
    let counters = runner.state().objects[&sphinx]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        counters, 1,
        "Rescuer Sphinx enters with a +1/+1 counter on it"
    );
}

#[test]
fn rescuer_sphinx_decline_leaves_permanent_and_enters_without_counter() {
    let mut scenario = GameScenario::new();
    let sphinx = scenario
        .add_creature_to_hand(P0, "Rescuer Sphinx", 3, 2)
        .from_oracle_text(RESCUER_SPHINX_ORACLE)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let mut runner = scenario.build();

    enter_via_change_zone(runner.state_mut(), sphinx);

    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::ReplacementChoice { .. }
        ),
        "expected Rescuer Sphinx MayCost ReplacementChoice, got {:?}",
        runner.state().waiting_for
    );

    // Decline (index 1)
    runner
        .act(GameAction::ChooseReplacement { index: 1 })
        .expect("declining the return cost should resolve");

    assert_eq!(
        runner.state().objects[&bear].zone,
        Zone::Battlefield,
        "declining the cost leaves the nonland permanent on the battlefield"
    );
    assert_eq!(
        runner.state().objects[&sphinx].zone,
        Zone::Battlefield,
        "Rescuer Sphinx enters the battlefield"
    );
    let counters = runner.state().objects[&sphinx]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        counters, 0,
        "Rescuer Sphinx enters with no counters on decline"
    );
}
