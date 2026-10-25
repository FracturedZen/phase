//! Alchemy "draft a card from [this card]'s spellbook" (digital-only, no CR
//! entry). A spellbook is a fixed, card-specific list of card names. Drafting
//! means: reveal the list, the controller chooses one card, and that card is
//! created in a destination zone.
//!
//! This composes two existing building blocks rather than reinventing them:
//!
//! * the interactive choice pauses on [`WaitingFor::SpellbookDraft`] (resumed by
//!   `GameAction::SubmitSpellbookDraft` in `engine.rs`); and
//! * the card creation delegates to [`super::conjure`] — the chosen name is
//!   conjured into the destination, reusing the registry lookup, characteristic
//!   application, summoning-sickness reset and ETB zone-change emission.
//!
//! The spellbook list itself is not in the Oracle text — it is carried on the
//! source object (`GameObject::spellbook`, copied from
//! `CardFace::metadata.spellbook`); the resolver reads it from the source.

use crate::types::game_state::RandomDraw;
use rand::Rng;

use crate::types::ability::{
    ConjureCard, ConjureSource, Effect, EffectError, EffectKind, QuantityExpr, ResolvedAbility,
};
use crate::types::counter::CounterType;
use crate::types::events::GameEvent;
use crate::types::game_state::{GameState, WaitingFor};
use crate::types::zones::Zone;

/// Resolve [`Effect::DraftFromSpellbook`]: present the source card's spellbook
/// list for the controller to choose from. With an empty list (no spellbook
/// data) the draft is a no-op.
pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let Effect::DraftFromSpellbook {
        destination,
        tapped,
        random,
        face_down,
        enter_with_counters,
    } = &ability.effect
    else {
        return Err(EffectError::MissingParam("DraftFromSpellbook".to_string()));
    };

    // The spellbook list lives on the source object (from its card face). Read
    // it from the source; an empty/missing list resolves as a no-op rather than
    // pausing on an empty choice.
    let spellbook = state
        .objects
        .get(&ability.source_id)
        .map(|obj| obj.spellbook.clone())
        .unwrap_or_default();

    if spellbook.is_empty() {
        events.push(GameEvent::EffectResolved {
            kind: EffectKind::from(&ability.effect),
            source_id: ability.source_id,
            subject: None,
        });
        return Ok(());
    }

    // "conjure a random card from ~'s spellbook": the engine picks one uniformly at random
    // and conjures it immediately, instead of pausing for the controller to choose. Random
    // vs. choice is the only difference from draft-from-spellbook, so both share this
    // resolver and the same card-creation path (`complete_draft`).
    if *random {
        let index = state
            .rng
            .draw(RandomDraw::Outcome)
            .random_range(0..spellbook.len());
        let card = spellbook[index].clone();
        return complete_draft(
            state,
            ability.controller,
            ability.source_id,
            &spellbook,
            &card,
            *destination,
            *tapped,
            *face_down,
            enter_with_counters.clone(),
            events,
        );
    }

    // Digital-only Alchemy spellbook choice: pause for the controller to choose
    // one card from the list.
    state.waiting_for = WaitingFor::SpellbookDraft {
        player: ability.controller,
        source_id: ability.source_id,
        options: spellbook,
        destination: *destination,
        tapped: *tapped,
        face_down: *face_down,
        enter_with_counters: enter_with_counters.clone(),
    };
    Ok(())
}

/// Complete a spellbook draft once the controller has chosen `card`: conjure the
/// chosen card into `destination`. Reuses [`super::conjure::resolve`] for the
/// actual card creation. `card` must be one of the offered `options`.
#[allow(clippy::too_many_arguments)]
pub fn complete_draft(
    state: &mut GameState,
    controller: crate::types::player::PlayerId,
    source_id: crate::types::identifiers::ObjectId,
    options: &[String],
    card: &str,
    destination: Zone,
    tapped: bool,
    face_down: bool,
    enter_with_counters: Vec<(CounterType, QuantityExpr)>,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    if !options.iter().any(|name| name == card) {
        return Err(EffectError::InvalidParam(format!(
            "{card} is not in the offered spellbook"
        )));
    }

    // Conjure the chosen card — one copy — into the destination, reusing the
    // single card-creation authority.
    let conjure = ResolvedAbility::new(
        Effect::Conjure {
            cards: vec![ConjureCard {
                source: ConjureSource::Named {
                    name: card.to_string(),
                },
                count: QuantityExpr::Fixed { value: 1 },
            }],
            destination,
            tapped,
            library_position: None,
            library_players: None,
        },
        Vec::new(),
        source_id,
        controller,
    );
    super::conjure::resolve(state, &conjure, events)?;

    // Digital-only Alchemy riders (Darigaaz, Shivan Champion #9871): apply
    // face-down concealment and entry counters to the just-conjured card(s).
    // `last_created_token_ids` is the chain referent Conjure just assigned.
    let created = state.last_created_token_ids.clone();
    if face_down {
        for id in &created {
            if let Some(obj) = state.objects.get_mut(id) {
                obj.face_down = true;
            }
        }
    }
    for (counter_type, count_expr) in &enter_with_counters {
        let count = match count_expr {
            QuantityExpr::Fixed { value } if *value >= 0 => *value as u32,
            _ => continue,
        };
        if count == 0 {
            continue;
        }
        for id in &created {
            let _ = super::counters::add_counter_with_replacement(
                state,
                controller,
                *id,
                counter_type.clone(),
                count,
                events,
            );
        }
    }

    Ok(())
}
