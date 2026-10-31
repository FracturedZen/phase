//! Issue #7355 — Disa the Restless: "Whenever a Lhurgoyf permanent card is put
//! into your graveyard from anywhere other than the battlefield, put it onto the
//! battlefield." CR 603.6c + CR 603.10a: the trigger must fire for a Lhurgoyf
//! card milled from the library, and must NOT fire for a Lhurgoyf that dies
//! (battlefield → graveyard). Previously the negated-origin clause parsed as
//! `TriggerMode::Unknown`.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::triggers::TriggerMode;
use engine::types::zones::Zone;

// The trailing "{1}: Mill a card." is test scaffolding to drive a
// library → graveyard move; Disa's own printed text is the first line.
const DISA_ORACLE: &str = "Whenever a Lhurgoyf permanent card is put into your \
graveyard from anywhere other than the battlefield, put it onto the battlefield.\n\
{1}: Mill a card.";

fn drain_stack(runner: &mut GameRunner) {
    for _ in 0..200 {
        if matches!(runner.state().waiting_for, WaitingFor::OrderTriggers { .. }) {
            engine::game::triggers::drain_order_triggers_with_identity(runner.state_mut());
            continue;
        }
        match &runner.state().waiting_for {
            WaitingFor::Priority { .. } if runner.state().stack.is_empty() => break,
            _ => {
                if runner.act(GameAction::PassPriority).is_err() {
                    break;
                }
            }
        }
    }
}

fn make_lhurgoyf(runner: &mut GameRunner, zone: Zone) -> ObjectId {
    let state = runner.state_mut();
    let card_id = CardId(state.next_object_id);
    let id = engine::game::zones::create_object(state, card_id, P0, "Goyf".to_string(), zone);
    let obj = state.objects.get_mut(&id).unwrap();
    obj.card_types.core_types.push(CoreType::Creature);
    obj.card_types.subtypes.push("Lhurgoyf".to_string());
    obj.base_card_types = obj.card_types.clone();
    id
}

fn setup() -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let disa = scenario
        .add_creature_from_oracle(P0, "Disa the Restless", 5, 6, DISA_ORACLE)
        .id();
    (scenario.build(), disa)
}

#[test]
fn disa_parses_negated_origin_trigger() {
    let (runner, disa) = setup();
    let obj = &runner.state().objects[&disa];
    let entry = obj
        .trigger_definitions
        .iter_unchecked()
        .next()
        .expect("Disa has a trigger");
    assert_eq!(entry.definition.mode, TriggerMode::ChangesZone);
}

#[test]
fn disa_returns_milled_lhurgoyf_to_battlefield() {
    let (mut runner, disa) = setup();
    {
        let p = &mut runner.state_mut().players[0];
        let ids: Vec<_> = p.library.iter().copied().collect();
        p.library.clear();
        for id in ids {
            runner.state_mut().objects.remove(&id);
        }
    }
    let goyf = make_lhurgoyf(&mut runner, Zone::Library);
    runner.state_mut().players[0].mana_pool.add(ManaUnit::new(
        ManaType::Colorless,
        ObjectId(0),
        false,
        vec![],
    ));
    runner
        .act(GameAction::ActivateAbility {
            source_id: disa,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("activate mill");
    drain_stack(&mut runner);
    assert_eq!(
        runner.state().objects[&goyf].zone,
        Zone::Battlefield,
        "milled Lhurgoyf must be put onto the battlefield"
    );
}

#[test]
fn disa_does_not_return_lhurgoyf_that_died() {
    let (mut runner, _disa) = setup();
    let goyf = make_lhurgoyf(&mut runner, Zone::Battlefield);
    {
        let obj = runner.state_mut().objects.get_mut(&goyf).unwrap();
        obj.toughness = Some(1);
        obj.power = Some(1);
        obj.damage_marked = 2;
    }
    let mut events = Vec::new();
    engine::game::sba::check_state_based_actions(runner.state_mut(), &mut events);
    engine::game::triggers::process_triggers(runner.state_mut(), &events);
    drain_stack(&mut runner);
    assert_eq!(
        runner.state().objects[&goyf].zone,
        Zone::Graveyard,
        "a Lhurgoyf that dies is not from 'anywhere other than the battlefield'"
    );
}
