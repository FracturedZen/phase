//! Issue #8131 — Omarthis, Ghostfire Initiate: "Whenever you put one or more
//! +1/+1 counters on another colorless creature, you may put a +1/+1 counter
//! on Omarthis."
//!
//! CR 603.2c: an ability triggers once for each time its trigger event occurs.
//! The event is counters being put on ONE other colorless creature, so a single
//! effect that puts counters on three colorless creatures at once is three
//! trigger events — Omarthis triggers three times.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;

const OMARTHIS: &str = "Omarthis enters with X +1/+1 counters on it.\nWhenever you put one or more +1/+1 counters on another colorless creature, you may put a +1/+1 counter on Omarthis.\nWhen Omarthis dies, manifest a number of cards from the top of your library equal to the number of counters on it.";

fn plus_counters(runner: &GameRunner, id: ObjectId) -> u32 {
    runner.state().objects[&id]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0)
}

fn drive(runner: &mut GameRunner) -> usize {
    let mut accepted = 0;
    for _ in 0..80 {
        let result = match runner.state().waiting_for.clone() {
            WaitingFor::Priority { .. } if runner.state().stack.is_empty() => return accepted,
            WaitingFor::Priority { .. } => runner.act(GameAction::PassPriority),
            WaitingFor::OrderTriggers { triggers, .. } => runner.act(GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            }),
            WaitingFor::OptionalEffectChoice { .. } => {
                accepted += 1;
                runner.act(GameAction::DecideOptionalEffect { accept: true })
            }
            WaitingFor::ManifestDreadChoice { cards, .. } => {
                runner.act(GameAction::SelectCards { cards: vec![cards[0]] })
            }
            other => panic!("unexpected prompt: {other:?}"),
        };
        result.expect("scripted answer is legal");
    }
    panic!("never settled");
}

#[test]
fn counters_on_three_colorless_creatures_at_once_trigger_three_times() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let omarthis = scenario
        .add_creature_from_oracle(P0, "Omarthis, Ghostfire Initiate", 0, 0, OMARTHIS)
        .with_subtypes(vec!["Spirit", "Snake"])
        .id();
    for name in ["Construct A", "Construct B", "Construct C"] {
        scenario.add_creature(P0, name, 1, 1);
    }
    let spell = scenario
        .add_spell_to_hand_from_oracle(
            P0,
            "Mass Growth",
            false,
            "Put a +1/+1 counter on each creature you control.",
        )
        .id();
    let mut runner = scenario.build();
    // Keep Omarthis alive as a 0/0 for the test's duration.
    runner
        .state_mut()
        .objects
        .get_mut(&omarthis)
        .unwrap()
        .counters
        .insert(CounterType::Plus1Plus1, 1);
    drop(runner.cast(spell).commit());
    let accepted = drive(&mut runner);
    assert_eq!(accepted, 3, "one trigger per colorless creature");
    assert_eq!(plus_counters(&runner, omarthis), 5);
}

const ONSLAUGHT: &str =
    "Manifest dread X times, then put X +1/+1 counters on each of those creatures.";

fn add_onslaught(scenario: &mut GameScenario) -> ObjectId {
    for i in 0..8 {
        scenario.add_card_to_library_top(P0, &format!("Library {i}"));
    }
    let mut b = scenario.add_spell_to_hand_from_oracle(P0, "Valgavoth's Onslaught", false, ONSLAUGHT);
    b.with_mana_cost(ManaCost::Cost {
        shards: vec![ManaCostShard::X, ManaCostShard::X, ManaCostShard::Green],
        generic: 0,
    });
    b.id()
}

fn add_green(runner: &mut GameRunner, count: usize) {
    for _ in 0..count {
        let unit = ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]);
        runner.state_mut().players[0].mana_pool.add(unit);
    }
}

/// The reported board: Valgavoth's Onslaught manifests dread X times, then puts
/// X +1/+1 counters on each of those (face-down, colorless — CR 708.2a)
/// creatures in one simultaneous placement.
#[test]
fn valgavoths_onslaught_triggers_once_per_manifested_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let omarthis = scenario
        .add_creature_from_oracle(P0, "Omarthis, Ghostfire Initiate", 0, 0, OMARTHIS)
        .with_subtypes(vec!["Spirit", "Snake"])
        .id();
    let spell = add_onslaught(&mut scenario);
    let mut runner = scenario.build();
    runner
        .state_mut()
        .objects
        .get_mut(&omarthis)
        .unwrap()
        .counters
        .insert(CounterType::Plus1Plus1, 1);
    add_green(&mut runner, 8);
    drop(runner.cast(spell).x(3).commit());
    let accepted = drive(&mut runner);
    let manifested: Vec<ObjectId> = runner
        .state()
        .battlefield
        .iter()
        .copied()
        .filter(|id| runner.state().objects[id].face_down)
        .collect();
    for id in &manifested {
        assert_eq!(plus_counters(&runner, *id), 3);
    }
    assert_eq!(manifested.len(), 3);
    assert_eq!(accepted, 3, "one trigger per manifested colorless creature");
    assert_eq!(plus_counters(&runner, omarthis), 4);
}

/// CR 603.2 + CR 603.6a: the same paused-iteration seam carries each
/// manifested creature's battlefield entry. Every one of the three entries
/// triggers an enters watcher exactly once — none lost to the next iteration's
/// choice, none collected twice.
#[test]
fn valgavoths_onslaught_each_manifest_entry_triggers_enters_watcher_once() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_enchantment_from_oracle(
        P0,
        "Entry Watcher",
        "Whenever a creature you control enters, you gain 1 life.",
    );
    let spell = add_onslaught(&mut scenario);
    let mut runner = scenario.build();
    add_green(&mut runner, 8);
    let life_before = runner.state().players[0].life;
    drop(runner.cast(spell).x(3).commit());
    drive(&mut runner);
    assert_eq!(runner.state().players[0].life, life_before + 3);
}
