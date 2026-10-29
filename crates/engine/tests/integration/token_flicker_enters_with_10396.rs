//! Issue #10396 — Ephemerate on a token must not put the token's enters-with
//! counters on it while it sits in exile.
//!
//! CR 111.8: a token that has left the battlefield can't move to another zone
//! or come back onto the battlefield; if it would, it stays where it is and
//! ceases to exist at the next state-based-action check (CR 704.5d). Because
//! the token never enters, no "enters with" replacement applies (CR 614.1c,
//! CR 614.12), so no counters are put on it and no `CounterAdded` fires that a
//! "whenever one or more counters are put on" trigger could see.
//!
//! https://github.com/phase-rs/phase/issues/10396

use engine::game::scenario::{GameScenario, P0};
use engine::types::counter::CounterType;
use engine::types::events::GameEvent;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

// Verbatim Oracle text (Scryfall).
const EPHEMERATE: &str = "Exile target creature you control, then return it to the battlefield under its owner's control.\nRebound (If you cast this spell from your hand, exile it as it resolves. At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.)";
const SPIKE_FEEDER: &str = "This creature enters with two +1/+1 counters on it.\n{2}, Remove a +1/+1 counter from this creature: Put a +1/+1 counter on target creature.\nRemove a +1/+1 counter from this creature: You gain 2 life.";

fn white(n: usize) -> Vec<ManaUnit> {
    (0..n)
        .map(|_| ManaUnit::new(ManaType::White, ObjectId(0), false, vec![]))
        .collect()
}

/// Blink a Spike Feeder (token or not) with Ephemerate and return the
/// creature's id, its zone, its +1/+1 counters, and the cast's events.
fn blink_spike_feeder(as_token: bool) -> (ObjectId, Option<Zone>, u32, Vec<GameEvent>) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, white(2));
    let feeder = scenario
        .add_creature_from_oracle(P0, "Spike Feeder", 0, 0, SPIKE_FEEDER)
        .id();
    let ephemerate = scenario
        .add_spell_to_hand_from_oracle(P0, "Ephemerate", true, EPHEMERATE)
        .id();
    let mut runner = scenario.build();
    runner
        .state_mut()
        .objects
        .get_mut(&feeder)
        .unwrap()
        .is_token = as_token;
    // Spike Feeder is 0/0 printed; give the board copy its counters so it
    // survives SBA before the blink, as it would have on its real entry.
    runner
        .state_mut()
        .objects
        .get_mut(&feeder)
        .unwrap()
        .counters
        .insert(CounterType::Plus1Plus1, 2);

    let outcome = runner.cast(ephemerate).target_object(feeder).resolve();
    let events = outcome.events().to_vec();
    assert!(
        events.iter().any(|e| matches!(
            e,
            GameEvent::ZoneChanged { object_id, to: Zone::Exile, .. } if *object_id == feeder
        )),
        "reach-guard: Ephemerate must exile the creature (blink leg 1)"
    );
    let zone = runner.state().objects.get(&feeder).map(|o| o.zone);
    let counters = runner
        .state()
        .objects
        .get(&feeder)
        .and_then(|o| o.counters.get(&CounterType::Plus1Plus1).copied())
        .unwrap_or(0);
    (feeder, zone, counters, events)
}

fn counters_added_to(events: &[GameEvent], id: ObjectId) -> u32 {
    events
        .iter()
        .filter_map(|e| match e {
            GameEvent::CounterAdded {
                object_id, count, ..
            } if *object_id == id => Some(*count),
            _ => None,
        })
        .sum()
}

#[test]
fn ephemerate_on_a_token_puts_no_enters_with_counters_on_it() {
    let (feeder, zone, _counters, events) = blink_spike_feeder(true);

    assert_ne!(
        zone,
        Some(Zone::Battlefield),
        "CR 111.8: an exiled token can't return to the battlefield"
    );
    assert!(
        !events.iter().any(|e| matches!(
            e,
            GameEvent::ZoneChanged { object_id, to: Zone::Battlefield, .. } if *object_id == feeder
        )),
        "CR 111.8: the token must not re-enter"
    );
    assert_eq!(
        counters_added_to(&events, feeder),
        0,
        "CR 111.8 + CR 614.12: no entry happens, so the enters-with replacement \
         must not put counters on the token in exile; events: {events:?}"
    );
}

/// Control: the same blink on a nontoken Spike Feeder returns it, and it
/// enters with its two +1/+1 counters (CR 614.1c) — the guard refuses only the
/// token's no-op move.
#[test]
fn ephemerate_on_a_nontoken_still_enters_with_counters() {
    let (feeder, zone, counters, events) = blink_spike_feeder(false);

    assert_eq!(zone, Some(Zone::Battlefield));
    assert_eq!(counters_added_to(&events, feeder), 2);
    assert_eq!(counters, 2);
}
