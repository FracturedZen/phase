//! Issue #8120: Invisible Woman, Sue Storm creates one Wall for a single
//! "one or more +1/+1 counters on one or more other Heroes" event.
//!
//! CR 700.1: the ability's text defines the event it looks for. "One or more
//! counters" on "one or more other Heroes" is one happening.
//! CR 603.2c: the ability triggers once each time that event occurs.
//! CR 122.1: the counters placed by the producer spells are the markers that
//! event puts on those Heroes.
//! CR 608.2d: "you may" is accepted while the trigger resolves.

use engine::game::scenario::{GameScenario, P0};
use engine::types::counter::CounterType;
use engine::types::game_state::{GameState, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;

const SUE: &str = "Lifelink\nWhenever you put one or more +1/+1 counters on one or more other Heroes you control, you may create a 0/4 colorless Wall creature token with defender.";

const PUT_TWO_EACH: &str = "Put two +1/+1 counters on each creature you control.";

const PUT_EACH: &str = "Put a +1/+1 counter on each creature you control.";

fn add_sue(scenario: &mut GameScenario) -> ObjectId {
    scenario
        .add_creature_from_oracle(P0, "Invisible Woman, Sue Storm", 2, 3, SUE)
        .with_subtypes(vec!["Human", "Hero"])
        .id()
}

fn add_hero(scenario: &mut GameScenario, name: &str) -> ObjectId {
    scenario
        .add_creature(P0, name, 2, 2)
        .with_subtypes(vec!["Human", "Hero"])
        .id()
}

fn wall_tokens(state: &GameState) -> Vec<ObjectId> {
    state
        .battlefield
        .iter()
        .copied()
        .filter(|id| {
            let object = &state.objects[id];
            object.is_token && object.controller == P0 && object.name == "Wall"
        })
        .collect()
}

fn assert_one_printed_wall(state: &GameState, walls: &[ObjectId]) {
    assert_eq!(walls.len(), 1, "expected one Wall token");
    let wall = &state.objects[&walls[0]];
    assert_eq!(wall.power, Some(0), "the printed Wall is 0/4");
    assert_eq!(wall.toughness, Some(4), "the printed Wall is 0/4");
    assert!(
        wall.card_types
            .subtypes
            .iter()
            .any(|subtype| subtype == "Wall"),
        "the token's subtype is Wall, got {:?}",
        wall.card_types.subtypes
    );
    assert!(
        wall.keywords
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Defender)),
        "the token has defender, got {:?}",
        wall.keywords
    );
}

/// Two +1/+1 counters put on one other Hero by one spell are one event, so
/// accepting the "you may" creates one Wall.
///
/// Single-recipient control. A per-counter reading would create one Wall for
/// each counter. A trigger that never fires leaves the count at 0.
#[test]
fn invisible_woman_one_counter_batch_on_one_hero_creates_one_wall() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let sue = add_sue(&mut scenario);
    let hero = add_hero(&mut scenario, "Heroic Friend");
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Put Two Each", false, PUT_TWO_EACH)
        .with_mana_cost(ManaCost::zero())
        .id();
    let mut runner = scenario.build();

    let outcome = runner.cast(spell).accept_optional().resolve();

    assert_eq!(
        outcome.counters(hero, CounterType::Plus1Plus1),
        2,
        "the producer must put both counters before the trigger is counted, waiting {:?}",
        outcome.final_waiting_for()
    );
    assert_eq!(
        outcome.counters(sue, CounterType::Plus1Plus1),
        2,
        "reach-guard: Sue also received counters, and \"other\" must still keep her out of the event's Hero set"
    );
    let walls = wall_tokens(outcome.state());
    assert_eq!(
        walls.len(),
        1,
        "one counter-placement event creates one Wall, found {walls:?}, waiting {:?}",
        outcome.final_waiting_for()
    );
    assert_one_printed_wall(outcome.state(), &walls);
    assert!(
        outcome.state().stack.is_empty(),
        "the may-trigger must finish"
    );
    assert!(
        matches!(outcome.final_waiting_for(), WaitingFor::Priority { .. }),
        "resolution must return to priority, waiting {:?}",
        outcome.final_waiting_for()
    );
}

/// Counters put on two other Heroes by one instruction are still one event.
///
/// Revert-failing: one trigger per Hero creates two Walls.
#[test]
fn invisible_woman_counters_on_two_heroes_at_once_create_one_wall() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let sue = add_sue(&mut scenario);
    let hero_a = add_hero(&mut scenario, "Hero A");
    let hero_b = add_hero(&mut scenario, "Hero B");
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Put Each", false, PUT_EACH)
        .with_mana_cost(ManaCost::zero())
        .id();
    let mut runner = scenario.build();

    let outcome = runner.cast(spell).accept_optional().resolve();

    assert_eq!(outcome.counters(hero_a, CounterType::Plus1Plus1), 1);
    assert_eq!(outcome.counters(hero_b, CounterType::Plus1Plus1), 1);
    assert_eq!(
        outcome.counters(sue, CounterType::Plus1Plus1),
        1,
        "reach-guard: Sue also received a counter, and \"other\" must still keep her out of the event's Hero set"
    );
    let walls = wall_tokens(outcome.state());
    assert_eq!(
        walls.len(),
        1,
        "counters on several other Heroes in one event create one Wall, found {walls:?}, waiting {:?}",
        outcome.final_waiting_for()
    );
    assert_one_printed_wall(outcome.state(), &walls);
    assert!(outcome.state().stack.is_empty());
    assert!(
        matches!(outcome.final_waiting_for(), WaitingFor::Priority { .. }),
        "resolution must return to priority, waiting {:?}",
        outcome.final_waiting_for()
    );
}
