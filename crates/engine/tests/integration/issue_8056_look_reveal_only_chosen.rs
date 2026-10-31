//! "Look at the top N ... you may reveal [a card] from among them and put it
//! into your hand" (Narset, Parter of Veils; Lead the Stampede) must reveal
//! only the chosen cards (CR 701.20a + CR 701.20e). The looked-at cards stay
//! private to their controller while choosing, and the cards not chosen are
//! never revealed. "Reveal the top N" spells still reveal all N.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::visibility::{filter_events_for_viewer, filter_state_for_viewer};
use engine::types::actions::GameAction;
use engine::types::events::GameEvent;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const LEAD: &str = "Look at the top five cards of your library. You may reveal any number of creature cards from among them and put the revealed cards into your hand. Put the rest on the bottom of your library in any order.";

const NARSET_MINUS_TWO: &str = "Look at the top four cards of your library. You may reveal a noncreature, nonland card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.";

const RUMBLE: &str = "Reveal the top four cards of your library. You may put a permanent card from among them into your hand. Put the rest into your graveyard.";

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

/// Every card id the opponent is told was revealed by these events.
fn revealed_to_opponent(runner: &GameRunner, events: &[GameEvent]) -> Vec<ObjectId> {
    filter_events_for_viewer(events, runner.state(), P1)
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardsRevealed { card_ids, .. } => Some(card_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn lead_the_stampede_reveals_only_the_chosen_creatures() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let skipped = scenario
        .add_spell_to_library_top(P0, "Skipped Beast", false)
        .as_creature()
        .id();
    let kept = scenario
        .add_spell_to_library_top(P0, "Kept Beast", false)
        .as_creature()
        .id();
    let mut others = Vec::new();
    for name in ["Other A", "Other B", "Other C"] {
        others.push(scenario.add_spell_to_library_top(P0, name, true).id());
    }
    let lead = scenario
        .add_spell_to_hand_from_oracle(P0, "Lead the Stampede", false, LEAD)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, lead);
    // CR 701.20e: the looked-at cards are private while the caster chooses.
    for id in [kept, skipped].iter().chain(&others) {
        assert_eq!(opponent_name(&runner, *id), "Hidden Card");
    }

    let result = runner
        .act(GameAction::SelectCards { cards: vec![kept] })
        .expect("reveal and keep one creature");
    assert_eq!(runner.state().objects[&kept].zone, Zone::Hand);

    // CR 701.20a: exactly the chosen card is revealed to the opponent.
    assert_eq!(revealed_to_opponent(&runner, &result.events), vec![kept]);
    // The creature that was looked at but not chosen is never shown.
    assert_ne!(runner.state().objects[&skipped].zone, Zone::Hand);
    assert_eq!(opponent_name(&runner, skipped), "Hidden Card");
    for id in &others {
        assert_eq!(opponent_name(&runner, *id), "Hidden Card");
    }
}

#[test]
fn narset_minus_two_reveals_only_the_chosen_card() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let other_spell = scenario
        .add_spell_to_library_top(P0, "Other Spell", true)
        .id();
    let kept = scenario
        .add_spell_to_library_top(P0, "Kept Spell", true)
        .id();
    let land = scenario.add_land_to_library_top(P0, "Some Land").id();
    let creature = scenario
        .add_spell_to_library_top(P0, "Some Creature", false)
        .as_creature()
        .id();
    let narset = scenario
        .add_spell_to_hand_from_oracle(P0, "Narset Minus Two", true, NARSET_MINUS_TWO)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, narset);
    for id in [other_spell, kept, land, creature] {
        assert_eq!(opponent_name(&runner, id), "Hidden Card");
    }

    let result = runner
        .act(GameAction::SelectCards { cards: vec![kept] })
        .expect("reveal and keep the spell");
    assert_eq!(runner.state().objects[&kept].zone, Zone::Hand);
    assert_eq!(revealed_to_opponent(&runner, &result.events), vec![kept]);
    for id in [other_spell, land, creature] {
        assert_eq!(opponent_name(&runner, id), "Hidden Card");
    }
}

#[test]
fn choosing_nothing_reveals_nothing() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let mut looked = Vec::new();
    for name in ["Spell A", "Spell B", "Spell C", "Spell D"] {
        looked.push(scenario.add_spell_to_library_top(P0, name, true).id());
    }
    let narset = scenario
        .add_spell_to_hand_from_oracle(P0, "Narset Minus Two", true, NARSET_MINUS_TWO)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, narset);
    let result = runner
        .act(GameAction::SelectCards { cards: vec![] })
        .expect("decline");
    assert!(revealed_to_opponent(&runner, &result.events).is_empty());
    for id in looked {
        assert_ne!(runner.state().objects[&id].zone, Zone::Hand);
        assert_eq!(opponent_name(&runner, id), "Hidden Card");
    }
}

#[test]
fn reveal_the_top_n_still_reveals_all_of_them() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let mut revealed = vec![scenario.add_land_to_library_top(P0, "Kept Land").id()];
    for name in ["Rest A", "Rest B", "Rest C"] {
        revealed.push(scenario.add_spell_to_library_top(P0, name, true).id());
    }
    let rumble = scenario
        .add_spell_to_hand_from_oracle(P0, "Malevolent Rumble", false, RUMBLE)
        .id();
    let mut runner = scenario.build();

    cast_until_dig_choice(&mut runner, rumble);
    // CR 701.20a: every revealed card is public while the caster chooses.
    for id in &revealed {
        assert_eq!(
            opponent_name(&runner, *id),
            runner.state().objects[id].name,
            "a card revealed from the top must be visible to the opponent"
        );
    }
}
