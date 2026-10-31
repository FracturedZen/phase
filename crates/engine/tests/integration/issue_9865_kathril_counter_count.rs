//! Runtime regression test for Kathril, Aspect Warper (issue #9865):
//!
//! "When Kathril enters, put a flying counter on any creature you control if a
//! creature card in your graveyard has flying. Repeat this process for first strike,
//! double strike, deathtouch, hexproof, indestructible, lifelink, menace, reach,
//! trample, and vigilance. Then put a +1/+1 counter on Kathril for each counter put
//! on a creature this way."
//!
//! CR 122.1: "A counter is a marker placed on an object or player... Counters are
//! not objects and have no characteristics of their own."
//! CR 608.2c: "The controller of the spell or ability follows its instructions in
//! the order written."
//! Official Card Ruling (2020-04-17):
//! "Kathril's ability counts the number of counters put on creatures, not the
//! number of creatures that had counters put on them. If you put a flying counter
//! and a first strike counter on the same creature, Kathril will get two +1/+1 counters."
//!
//! Prior to the fix, "for each counter put on a creature this way" fell through to
//! `QuantityRef::TrackedSetSize`, which counted distinct creature object IDs in the
//! tracked set rather than the total count of counters placed across the resolution.
//! Placing multiple keyword counters on the same creature therefore resulted in
//! fewer +1/+1 counters than required by CR 122.1.

use engine::game::scenario::{GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;

const KATHRIL_ORACLE: &str = "When Kathril enters, put a flying counter on any creature you control if a creature card in your graveyard has flying. Repeat this process for first strike, double strike, deathtouch, hexproof, indestructible, lifelink, menace, reach, trample, and vigilance. Then put a +1/+1 counter on Kathril for each counter put on a creature this way.";

#[test]
fn test_kathril_counts_counters_when_multiple_put_on_same_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario
        .add_creature_to_graveyard(P0, "Skybound Remains", 1, 1)
        .flying();
    scenario
        .add_creature_to_graveyard(P0, "Duelist Remains", 1, 1)
        .first_strike();
    let bears = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let kathril = scenario
        .add_creature_to_hand_from_oracle(P0, "Kathril, Aspect Warper", 3, 3, KATHRIL_ORACLE)
        .id();

    scenario.with_mana_pool(
        P0,
        [
            ManaType::White,
            ManaType::Black,
            ManaType::Green,
            ManaType::Colorless,
            ManaType::Colorless,
        ]
        .into_iter()
        .zip(9_990u64..)
        .map(|(mana, id)| ManaUnit::new(mana, ObjectId(id), false, vec![]))
        .collect(),
    );

    let mut runner = scenario.build();
    runner.cast(kathril).commit();

    // Both flying and first strike counters are placed on Kathril.
    let mut recipients = [kathril, kathril].into_iter();
    let mut recipient_prompts = 0;
    for _ in 0..32 {
        let action = match &runner.state().waiting_for {
            WaitingFor::Priority { .. } if runner.state().stack.is_empty() => break,
            WaitingFor::Priority { .. } => GameAction::PassPriority,
            WaitingFor::ChooseFromZoneChoice { .. } => {
                recipient_prompts += 1;
                GameAction::SelectCards {
                    cards: vec![recipients.next().expect("two recipient prompts")],
                }
            }
            other => panic!("unexpected prompt {other:?}"),
        };
        runner.act(action).expect("action accepted");
    }

    let count_of = |id: ObjectId, counter: CounterType| {
        runner.state().objects[&id]
            .counters
            .get(&counter)
            .copied()
            .unwrap_or(0)
    };

    assert_eq!(recipient_prompts, 2);
    assert_eq!(
        count_of(bears, CounterType::Keyword(Keyword::Flying.kind())),
        0
    );
    assert_eq!(
        count_of(bears, CounterType::Keyword(Keyword::FirstStrike.kind())),
        0
    );
    assert_eq!(
        count_of(kathril, CounterType::Keyword(Keyword::Flying.kind())),
        1
    );
    assert_eq!(
        count_of(kathril, CounterType::Keyword(Keyword::FirstStrike.kind())),
        1
    );
    // CR 122.1: Kathril gets 2 +1/+1 counters because 2 total counters were put on creatures this way.
    assert_eq!(count_of(kathril, CounterType::Plus1Plus1), 2);
}

#[test]
fn test_kathril_counts_counters_distributed_across_multiple_creatures() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario
        .add_creature_to_graveyard(P0, "Skybound Remains", 1, 1)
        .flying();
    scenario
        .add_creature_to_graveyard(P0, "Duelist Remains", 1, 1)
        .first_strike();
    scenario
        .add_creature_to_graveyard(P0, "Venomous Remains", 1, 1)
        .deathtouch();
    let bears = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let kathril = scenario
        .add_creature_to_hand_from_oracle(P0, "Kathril, Aspect Warper", 3, 3, KATHRIL_ORACLE)
        .id();

    scenario.with_mana_pool(
        P0,
        [
            ManaType::White,
            ManaType::Black,
            ManaType::Green,
            ManaType::Colorless,
            ManaType::Colorless,
        ]
        .into_iter()
        .zip(9_990u64..)
        .map(|(mana, id)| ManaUnit::new(mana, ObjectId(id), false, vec![]))
        .collect(),
    );

    let mut runner = scenario.build();
    runner.cast(kathril).commit();

    // 3 counters: Flying -> Kathril, First Strike -> Kathril, Deathtouch -> Bears.
    // 2 counters on Kathril, 1 on Bears = 3 total counters put on creatures.
    let mut recipients = [kathril, kathril, bears].into_iter();
    let mut recipient_prompts = 0;
    for _ in 0..32 {
        let action = match &runner.state().waiting_for {
            WaitingFor::Priority { .. } if runner.state().stack.is_empty() => break,
            WaitingFor::Priority { .. } => GameAction::PassPriority,
            WaitingFor::ChooseFromZoneChoice { .. } => {
                recipient_prompts += 1;
                GameAction::SelectCards {
                    cards: vec![recipients.next().expect("three recipient prompts")],
                }
            }
            other => panic!("unexpected prompt {other:?}"),
        };
        runner.act(action).expect("action accepted");
    }

    let count_of = |id: ObjectId, counter: CounterType| {
        runner.state().objects[&id]
            .counters
            .get(&counter)
            .copied()
            .unwrap_or(0)
    };

    assert_eq!(recipient_prompts, 3);
    assert_eq!(
        count_of(kathril, CounterType::Keyword(Keyword::Flying.kind())),
        1
    );
    assert_eq!(
        count_of(kathril, CounterType::Keyword(Keyword::FirstStrike.kind())),
        1
    );
    assert_eq!(
        count_of(bears, CounterType::Keyword(Keyword::Deathtouch.kind())),
        1
    );
    // CR 122.1: Kathril gets 3 +1/+1 counters (2 on Kathril + 1 on Bears = 3 total counters put this way).
    assert_eq!(count_of(kathril, CounterType::Plus1Plus1), 3);
}
