//! CR 611.2a + CR 701.15a + CR 508.1c + CR 509.1b + CR 109.5 (issue #9608):
//! Immortal Obligation returns the targeted creature under its owner's control
//! with a duty counter, and while that counter is on it the creature is goaded
//! by the caster and cannot block creatures that caster controls.

use crate::game::combat::{can_block_pair, goading_players_for_creature};
use crate::game::layers::evaluate_layers;
use crate::game::scenario::{GameScenario, P0, P1};
use crate::types::ability::{ContinuousModification, Duration, StaticCondition, TargetFilter};
use crate::types::counter::{CounterMatch, CounterType};
use crate::types::game_state::{GameState, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use crate::types::phase::Phase;
use crate::types::statics::StaticMode;
use crate::types::triggers::AttackTargetFilter;
use crate::types::zones::Zone;

const ORACLE: &str = "Return target creature card from an opponent's graveyard to the battlefield \
under their control with a duty counter on it. For as long as that creature has a duty counter \
on it, it is goaded, can't attack you or a permanent you control, and can't block creatures you \
control.";

fn duty_counter() -> CounterType {
    CounterType::Generic("duty".to_string())
}

fn battlefield_named(state: &GameState, staged: ObjectId, name: &str) -> ObjectId {
    if state.objects.get(&staged).is_some_and(|obj| {
        obj.zone == Zone::Battlefield && obj.name == name
    }) {
        return staged;
    }
    state
        .objects
        .values()
        .find(|obj| obj.zone == Zone::Battlefield && obj.name == name)
        .map(|obj| obj.id)
        .unwrap_or_else(|| panic!("{name} must be on the battlefield"))
}

#[test]
fn immortal_obligation_duty_rider_uses_the_caster_as_you() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Immortal Obligation", true, ORACLE)
        .with_mana_cost(ManaCost::Cost {
            generic: 1,
            shards: vec![ManaCostShard::White],
        })
        .id();
    let exilee = scenario
        .add_creature_to_graveyard(P1, "Exilee", 2, 2)
        .id();
    let ally = scenario.add_creature(P0, "Ally", 2, 2).id();
    let other = scenario.add_creature(P1, "Other", 2, 2).id();
    scenario.with_mana_pool(
        P0,
        vec![
            ManaUnit::new(ManaType::White, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::White, ObjectId(0), false, vec![]),
        ],
    );
    let mut runner = scenario.build();

    let outcome = runner.cast(spell).target_objects(&[exilee]).resolve();
    assert!(
        matches!(outcome.final_waiting_for(), WaitingFor::Priority { .. }),
        "the spell must finish at priority: {:?}",
        outcome.final_waiting_for()
    );
    assert!(
        runner.state().stack.is_empty(),
        "resolution must leave the stack empty: {:?}",
        runner.state().stack
    );

    let duty = battlefield_named(runner.state(), exilee, "Exilee");
    let returned = &runner.state().objects[&duty];
    assert_eq!(returned.controller, P1);
    assert_eq!(returned.owner, P1);
    assert_eq!(returned.counters.get(&duty_counter()).copied(), Some(1));
    assert!(
        returned.goaded_by.is_empty(),
        "the designation is the continuous effect, not a one-shot goad: {:?}",
        returned.goaded_by
    );
    let goaders = goading_players_for_creature(runner.state(), duty);
    assert!(
        goaders.contains(&P0),
        "the caster goads the returned creature: {goaders:?}"
    );

    let tce = runner
        .state()
        .transient_continuous_effects
        .iter()
        .find(|tce| {
            matches!(tce.affected, TargetFilter::SpecificObject { id } if id == duty)
                && tce.modifications.iter().any(|modification| {
                    matches!(
                        modification,
                        ContinuousModification::AddStaticMode {
                            mode: StaticMode::Goaded
                        }
                    )
                })
        })
        .expect("a live duty-counter designation");
    assert_eq!(tce.controller, P0);
    assert!(
        matches!(
            &tce.duration,
            Duration::ForAsLongAs {
                condition: StaticCondition::RecipientHasCounters {
                    counters: CounterMatch::OfType(counter),
                    minimum: 1,
                    maximum: None,
                }
            } if *counter == duty_counter()
        ),
        "the designation lasts while the duty counter is there: {:?}",
        tce.duration
    );

    evaluate_layers(runner.state_mut());
    let grafted = &runner.state().objects[&duty];
    let attack = grafted
        .static_definitions
        .iter_all()
        .find(|def| matches!(def.mode, StaticMode::CantAttack))
        .expect("the attack prohibition is grafted onto the creature");
    assert_eq!(
        attack.attack_defended,
        Some(AttackTargetFilter::PlayerOrPermanents)
    );
    assert_eq!(attack.source_controller, Some(P0));
    let block = grafted
        .static_definitions
        .iter_all()
        .find(|def| matches!(def.mode, StaticMode::BlockRestriction { .. }))
        .expect("the block prohibition is grafted onto the creature");
    assert_eq!(block.source_controller, Some(P0));

    assert!(
        !can_block_pair(runner.state(), duty, ally),
        "the duty creature cannot block a creature the caster controls"
    );
    assert!(
        can_block_pair(runner.state(), duty, other),
        "the duty creature can still block its controller's other creature"
    );
    assert!(
        can_block_pair(runner.state(), other, ally),
        "the restriction is on the duty creature, not on every blocker"
    );

    runner
        .state_mut()
        .objects
        .get_mut(&duty)
        .expect("duty creature")
        .counters
        .remove(&duty_counter());
    evaluate_layers(runner.state_mut());
    assert!(
        !goading_players_for_creature(runner.state(), duty).contains(&P0),
        "removing the duty counter ends the goad"
    );
    assert!(
        can_block_pair(runner.state(), duty, ally),
        "removing the duty counter ends the block prohibition"
    );
}
