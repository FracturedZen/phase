//! CR 608.2c + CR 201.5 — Estinien Varlineau (#8161).
//!
//! Verbatim Oracle:
//!   Whenever you cast a noncreature spell, put a +1/+1 counter on Estinien
//!   Varlineau. It gains flying until end of turn.
//!   At the beginning of your second main phase, you draw X cards and lose X
//!   life, where X is the number of your opponents who were dealt combat damage
//!   by Estinien Varlineau or a Dragon this turn.
//!
//! The counter instruction names Estinien. The following "It" is that same
//! object. The spell that caused the trigger is an earlier antecedent, so the
//! flying grant resolves on Estinien while that spell is still on the stack.

use engine::game::keywords::object_has_effective_keyword_kind;
use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::counter::CounterType;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::KeywordKind;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const ESTINIEN_ORACLE: &str = "Whenever you cast a noncreature spell, put a +1/+1 counter on Estinien Varlineau. It gains flying until end of turn.\n\
At the beginning of your second main phase, you draw X cards and lose X life, where X is the number of your opponents who were dealt combat damage by Estinien Varlineau or a Dragon this turn.";

fn plus_one_counters(runner: &GameRunner, object: ObjectId) -> u32 {
    runner
        .state()
        .objects
        .get(&object)
        .and_then(|obj| obj.counters.get(&CounterType::Plus1Plus1).copied())
        .unwrap_or(0)
}

fn has_flying(runner: &GameRunner, object: ObjectId) -> bool {
    object_has_effective_keyword_kind(runner.state(), object, KeywordKind::Flying)
}

#[test]
fn estinien_noncreature_spell_gives_estinien_the_counter_and_flying() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let estinien = scenario
        .add_creature_from_oracle(P0, "Estinien Varlineau", 3, 3, ESTINIEN_ORACLE)
        .id();
    let bolt = scenario.add_bolt_to_hand(P0);
    let target = scenario.add_creature(P0, "Memnite", 1, 1).id();
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![])],
    );

    let mut runner = scenario.build();
    // CR 601.2i: the spell commits, and the cast trigger is above it.
    // Resolve that trigger while the spell is still on the stack.
    runner.cast(bolt).target_object(target).commit();
    runner.resolve_top();
    evaluate_layers(runner.state_mut());

    let state = runner.state();
    assert_eq!(
        state.objects[&bolt].zone,
        Zone::Stack,
        "the noncreature spell is still on the stack after its cast trigger resolves"
    );
    assert_eq!(state.stack.len(), 1);
    assert_eq!(state.stack[0].id, bolt);
    assert_eq!(
        plus_one_counters(&runner, estinien),
        1,
        "the counter instruction names Estinien Varlineau"
    );
    assert!(
        has_flying(&runner, estinien),
        "It gains flying refers to Estinien, the object named by the previous instruction"
    );
    assert!(
        !has_flying(&runner, bolt),
        "the cast spell keeps its own characteristics; flying was granted to Estinien"
    );
    assert_eq!(state.objects[&target].zone, Zone::Battlefield);
}

#[test]
fn estinien_creature_spell_does_not_give_the_counter_or_flying() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let estinien = scenario
        .add_creature_from_oracle(P0, "Estinien Varlineau", 3, 3, ESTINIEN_ORACLE)
        .id();
    let creature = scenario
        .add_creature_to_hand(P0, "Ornithopter", 0, 2)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    let outcome = runner.cast(creature).resolve();
    assert_eq!(outcome.zone_of(creature), Zone::Battlefield);
    evaluate_layers(runner.state_mut());

    assert_eq!(
        plus_one_counters(&runner, estinien),
        0,
        "a creature spell is outside Whenever you cast a noncreature spell"
    );
    assert!(
        !has_flying(&runner, estinien),
        "Estinien gains flying only from the noncreature-spell trigger"
    );
    assert_eq!(runner.state().stack.len(), 0);
}
