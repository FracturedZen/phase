//! Integration tests for Alacrian Armory (#10285):
//! "At the beginning of combat on your turn, choose up to one target Mount or
//! Vehicle you control. Until end of turn, that permanent becomes saddled if
//! it's a Mount and becomes an artifact creature if it's a Vehicle."
//!
//! CR 608.2c: later anaphors ("that permanent" / the empty-subject second
//! conjunct) name the announced Mount-or-Vehicle target.
//! CR 611.2a: the leading "Until end of turn" applies to both becomes conjuncts.
//! CR 115.6: if the optional target is declined, neither conjunct applies.
//!
//! Reported bug: the "Mount or Vehicle" Or-target was invisible to later
//! anaphors, so the Vehicle conjunct bound the Armory (SelfRef) and lost the
//! until-EOT duration.

use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameScenario, P0};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::phase::Phase;

const ALACRIAN_ARMORY: &str = "Creatures you control get +0/+1 and have vigilance.\nAt the beginning of combat on your turn, choose up to one target Mount or Vehicle you control. Until end of turn, that permanent becomes saddled if it's a Mount and becomes an artifact creature if it's a Vehicle.";

fn parse_armory_trigger_execute_debug() -> String {
    let parsed = parse_oracle_text(
        ALACRIAN_ARMORY,
        "Alacrian Armory",
        &[],
        &["Artifact".to_string()],
        &[],
    );
    let trigger = parsed
        .triggers
        .iter()
        .find(|t| t.execute.is_some())
        .expect("Alacrian Armory has a begin-combat trigger");
    format!("{:#?}", trigger.execute.as_ref().unwrap())
}

/// The Vehicle conjunct must name the chosen permanent (ParentTarget), not the
/// Armory (SelfRef), and must keep the printed until-EOT window.
#[test]
fn alacrian_armory_vehicle_conjunct_binds_parent_target_until_eot() {
    let dbg = parse_armory_trigger_execute_debug();

    assert!(
        !dbg.contains("SelfRef"),
        "the Vehicle conjunct must not bind the Armory itself (SelfRef); parsed execute:\n{dbg}"
    );
    assert!(
        dbg.contains("ParentTarget"),
        "the chosen Mount-or-Vehicle must bind ParentTarget for later anaphors (CR 608.2c); parsed execute:\n{dbg}"
    );
    assert!(
        dbg.contains("UntilEndOfTurn"),
        "the leading 'Until end of turn' must reach the becomes conjuncts (CR 611.2a); parsed execute:\n{dbg}"
    );
    assert!(
        dbg.contains("BecomeSaddled"),
        "the Mount conjunct must lower to BecomeSaddled; parsed execute:\n{dbg}"
    );
}

/// Gameplay repro: targeting a Vehicle must make THAT permanent an artifact
/// creature until EOT, and must not animate the Armory.
#[test]
fn alacrian_armory_targeted_vehicle_becomes_artifact_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let armory = scenario
        .add_artifact_from_oracle(P0, "Alacrian Armory", ALACRIAN_ARMORY)
        .id();
    let vehicle = {
        let mut b = scenario.add_artifact_from_oracle(P0, "Sky Skiff", "");
        b.with_subtypes(vec!["Vehicle"]);
        b.id()
    };

    let mut runner = scenario.build();
    runner.pass_both_players();

    assert_eq!(
        runner.state().phase,
        Phase::BeginCombat,
        "reach-guard: begin combat must have started"
    );
    runner
        .act(GameAction::SelectTargets {
            targets: vec![TargetRef::Object(vehicle)],
        })
        .unwrap_or_else(|err| {
            panic!(
                "select the Vehicle for Alacrian Armory; waiting_for={:?}, err={err:?}",
                runner.state().waiting_for
            )
        });
    runner.advance_until_stack_empty();
    evaluate_layers(runner.state_mut());

    let veh = &runner.state().objects[&vehicle];
    assert!(
        veh.card_types.core_types.contains(&CoreType::Artifact),
        "the Vehicle remains an artifact; types={:?}",
        veh.card_types.core_types
    );
    assert!(
        veh.card_types.core_types.contains(&CoreType::Creature),
        "targeted Vehicle must become an artifact creature until EOT (CR 608.2c + 611.2a); types={:?}",
        veh.card_types.core_types
    );

    let arm = &runner.state().objects[&armory];
    assert!(
        !arm.card_types.core_types.contains(&CoreType::Creature),
        "the Armory itself must not become a creature (SelfRef misbind); types={:?}",
        arm.card_types.core_types
    );
}

/// Mount path: the same trigger should saddle a targeted Mount (CR 702.171b).
#[test]
fn alacrian_armory_targeted_mount_becomes_saddled() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_artifact_from_oracle(P0, "Alacrian Armory", ALACRIAN_ARMORY);
    let mount = scenario
        .add_creature(P0, "Trusty Steed", 2, 2)
        .with_subtypes(vec!["Mount"])
        .id();

    let mut runner = scenario.build();
    runner.pass_both_players();
    runner
        .act(GameAction::SelectTargets {
            targets: vec![TargetRef::Object(mount)],
        })
        .unwrap_or_else(|err| {
            panic!(
                "select the Mount for Alacrian Armory; waiting_for={:?}, err={err:?}",
                runner.state().waiting_for
            )
        });
    runner.advance_until_stack_empty();

    assert!(
        runner.state().objects[&mount].is_saddled,
        "targeted Mount must become saddled until EOT"
    );
}
