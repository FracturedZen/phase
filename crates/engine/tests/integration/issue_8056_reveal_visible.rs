//! Issue #8056: a revealed card put into a hand must stay known to every
//! player (CR 701.20a). Malevolent Rumble reveals the top four cards, then the
//! caster keeps a permanent card after a choice; Lead the Stampede (the same
//! reveal-dig shape as Narset, Parter of Veils) reveals the cards it puts into
//! the hand. Before the fix the kept card turned back into "Hidden Card" for
//! opponents, its library-to-hand move was filtered out of their events, and
//! the shared log skipped it, so opponents could only infer the pick.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::visibility::{filter_events_for_viewer, filter_state_for_viewer};
use engine::types::actions::GameAction;
use engine::types::events::GameEvent;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::log::{GameLogEntry, LogSegment};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const RUMBLE: &str = "Reveal the top four cards of your library. You may put a permanent card from among them into your hand. Put the rest into your graveyard. Create a 0/1 colorless Eldrazi Spawn creature token with \"Sacrifice this token: Add {C}.\"";

const LEAD: &str = "Look at the top five cards of your library. You may reveal any number of creature cards from among them and put the revealed cards into your hand. Put the rest on the bottom of your library in any order.";

const PRIVATE_LOOK: &str = "Look at the top three cards of your library. Put one of them into your hand and the rest into your graveyard.";

fn cast_until_dig_choice(runner: &mut GameRunner, spell: ObjectId) {
    let card_id = runner.state().objects[&spell].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: spell,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast");
    for _ in 0..16 {
        if matches!(runner.state().waiting_for, WaitingFor::DigChoice { .. }) {
            return;
        }
        runner.act(GameAction::PassPriority).expect("pass");
    }
    panic!("expected a DigChoice, got {:?}", runner.state().waiting_for);
}

fn opponent_name(runner: &GameRunner, id: ObjectId) -> String {
    filter_state_for_viewer(runner.state(), P1).objects[&id]
        .name
        .clone()
}

fn opponent_sees_move_to_hand(runner: &GameRunner, events: &[GameEvent], id: ObjectId) -> bool {
    filter_events_for_viewer(events, runner.state(), P1)
        .iter()
        .any(|event| {
            matches!(
                event,
                GameEvent::ZoneChanged {
                    object_id,
                    from: Some(Zone::Library),
                    to: Zone::Hand,
                    ..
                } if *object_id == id
            )
        })
}

fn log_narrates_move_to_hand(entries: &[GameLogEntry], id: ObjectId) -> bool {
    entries.iter().any(|entry| {
        entry
            .segments
            .iter()
            .any(|s| matches!(s, LogSegment::CardName { object_id, .. } if *object_id == id))
            && entry
                .segments
                .iter()
                .any(|s| matches!(s, LogSegment::Zone(Zone::Hand)))
    })
}

#[test]
fn malevolent_rumble_kept_card_stays_known_to_opponents() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let kept = scenario.add_land_to_library_top(P0, "Kept Land").id();
    for name in ["Rest A", "Rest B", "Rest C"] {
        scenario.add_spell_to_library_top(P0, name, true);
    }
    let secret = scenario.add_card_to_hand(P0, "Secret Card");
    let rumble = scenario
        .add_spell_to_hand_from_oracle(P0, "Malevolent Rumble", false, RUMBLE)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, rumble);
    // CR 701.20a: all four revealed cards are public while the caster chooses.
    assert_eq!(opponent_name(&runner, kept), "Kept Land");

    let result = runner
        .act(GameAction::SelectCards { cards: vec![kept] })
        .expect("keep the land");
    assert_eq!(runner.state().objects[&kept].zone, Zone::Hand);

    assert_eq!(
        opponent_name(&runner, kept),
        "Kept Land",
        "the opponent watched the revealed card go to the hand"
    );
    assert!(
        opponent_sees_move_to_hand(&runner, &result.events, kept),
        "the revealed card's library-to-hand move must reach the opponent"
    );
    assert!(
        log_narrates_move_to_hand(&result.log_entries, kept),
        "the shared log must narrate the revealed card going to the hand: {:?}",
        result.log_entries
    );
    // CR 400.2: the rest of the hand stays hidden.
    assert_eq!(opponent_name(&runner, secret), "Hidden Card");

    // The knowledge outlives the reveal window.
    runner.act(GameAction::PassPriority).expect("pass");
    assert_eq!(opponent_name(&runner, kept), "Kept Land");
}

#[test]
fn reveal_from_among_cards_put_into_hand_stay_known_to_opponents() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let beast = scenario
        .add_spell_to_library_top(P0, "Revealed Beast", false)
        .as_creature()
        .id();
    for name in ["Other A", "Other B", "Other C", "Other D"] {
        scenario.add_spell_to_library_top(P0, name, true);
    }
    let lead = scenario
        .add_spell_to_hand_from_oracle(P0, "Lead the Stampede", false, LEAD)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, lead);
    let result = runner
        .act(GameAction::SelectCards { cards: vec![beast] })
        .expect("reveal and keep the creature");
    assert_eq!(runner.state().objects[&beast].zone, Zone::Hand);
    assert_eq!(opponent_name(&runner, beast), "Revealed Beast");
    assert!(opponent_sees_move_to_hand(&runner, &result.events, beast));
    assert!(log_narrates_move_to_hand(&result.log_entries, beast));
}

#[test]
fn privately_looked_at_card_put_into_hand_stays_hidden() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let kept = scenario.add_land_to_library_top(P0, "Private Land").id();
    for name in ["Look A", "Look B"] {
        scenario.add_spell_to_library_top(P0, name, true);
    }
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Private Look", false, PRIVATE_LOOK)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, spell);
    let result = runner
        .act(GameAction::SelectCards { cards: vec![kept] })
        .expect("keep");
    assert_eq!(runner.state().objects[&kept].zone, Zone::Hand);
    // CR 701.20e: a look is private; nothing about the pick reaches the opponent.
    assert_eq!(opponent_name(&runner, kept), "Hidden Card");
    assert!(!opponent_sees_move_to_hand(&runner, &result.events, kept));
    assert!(!log_narrates_move_to_hand(&result.log_entries, kept));
}
