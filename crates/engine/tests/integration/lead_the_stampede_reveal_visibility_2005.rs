//! Issue #2005 — Lead the Stampede: "When opponent resolves Lead the Stampede
//! and selects cards, I can't see the cards, but they have to be revealed."
//!
//! Lead the Stampede reads: "Look at the top five cards of your library. You
//! may reveal any number of creature cards from among them and put the revealed
//! cards into your hand. Put the rest on the bottom ...".
//!
//! CR 701.20a + CR 701.20e: the five cards are only *looked at* — only their
//! controller sees them while choosing. Only the creature cards the controller
//! chooses are revealed, and they are revealed to everyone at the keep. This
//! test was corrected from the upstream version, which expected all five
//! looked-at cards to be public during the `DigChoice` (an over-reveal).
//! The keep step is covered by `issue_8056_look_reveal_only_chosen`.

use engine::game::ability_utils::build_resolved_from_def;
use engine::game::effects::resolve_ability_chain;
use engine::game::visibility::filter_state_for_viewer;
use engine::game::zones::create_object;
use engine::parser::oracle_effect::parse_effect_chain;
use engine::types::ability::{AbilityKind, Effect};
use engine::types::card_type::CoreType;
use engine::types::events::GameEvent;
use engine::types::game_state::{GameState, WaitingFor};
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const LEAD_ORACLE: &str = "Look at the top five cards of your library. You may reveal \
    any number of creature cards from among them and put the revealed cards into your \
    hand. Put the rest on the bottom of your library in any order.";

fn library_creature(state: &mut GameState, card_id: u64, owner: PlayerId, name: &str) -> ObjectId {
    let oid = create_object(
        state,
        CardId(card_id),
        owner,
        name.to_string(),
        Zone::Library,
    );
    let obj = state.objects.get_mut(&oid).expect("just created");
    obj.card_types.core_types.push(CoreType::Creature);
    obj.base_card_types = obj.card_types.clone();
    oid
}

#[test]
fn lead_the_stampede_keeps_looked_at_cards_private_during_choice() {
    let def = parse_effect_chain(LEAD_ORACLE, AbilityKind::Spell);
    assert!(
        matches!(
            def.effect.as_ref(),
            Effect::Dig {
                reveal: false,
                reveal_chosen: true,
                ..
            }
        ),
        "Lead the Stampede must parse to a look-dig that reveals only the chosen cards: {:?}",
        def.effect
    );

    let mut state = GameState::new_two_player(2005);

    // Six creature cards in the caster's library: the top five are looked at.
    let lib: Vec<ObjectId> = (0..6)
        .map(|i| library_creature(&mut state, 100 + i, PlayerId(0), &format!("Beast {i}")))
        .collect();

    let source = create_object(
        &mut state,
        CardId(1),
        PlayerId(0),
        "Lead the Stampede".to_string(),
        Zone::Stack,
    );
    let ability = build_resolved_from_def(&def, source, PlayerId(0));
    let mut events = Vec::new();
    resolve_ability_chain(&mut state, &ability, &mut events, 0)
        .expect("Lead the Stampede resolves");

    let WaitingFor::DigChoice {
        cards,
        reveal_chosen,
        ..
    } = &state.waiting_for
    else {
        panic!(
            "look-dig must surface a DigChoice, got {:?}",
            state.waiting_for
        );
    };
    assert_eq!(cards.len(), 5, "the top five cards are looked at");
    assert!(*reveal_chosen, "the keep must reveal the chosen cards");

    // CR 701.20e: looking reveals nothing to other players.
    assert!(
        lib.iter().all(|id| !state.revealed_cards.contains(id)),
        "no looked-at card may be public before the choice"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, GameEvent::CardsRevealed { .. })),
        "the look must not emit a reveal: {events:?}"
    );

    // The controller knows the looked-at cards; the opponent sees none of them.
    let own_view = filter_state_for_viewer(&state, PlayerId(0));
    let opp_view = filter_state_for_viewer(&state, PlayerId(1));
    for id in cards {
        assert_eq!(own_view.objects[id].name, state.objects[id].name);
        assert_eq!(
            opp_view.objects[id].name, "Hidden Card",
            "the opponent must not see a looked-at card (CR 701.20e)"
        );
    }
}
