//! CR 111.11: `Effect::CreateTokenFromOracleCard` resolver. "Create a <Name>
//! token" where the name is not a predefined token type (CR 111.10) and the
//! effect defines no other characteristics: the token uses the card with that
//! name in the Oracle card reference to determine its characteristics (CR
//! 111.11's own example: Disa the Restless creating a Tarmogoyf token).
//!
//! The Oracle card exists only as a `CardFace` in `GameState::card_db`, so the
//! resolver builds `CopiableValues` from the face (mana cost, colors, types,
//! abilities, characteristic-defining abilities) and routes them through the
//! SHARED copy-token apply path (`token_copy::drive_copy_token_batches`), the
//! same path `CreateTokenCopyFromPool` uses, so CR 614 token replacements
//! (Doubling Season and friends) apply unchanged.

use crate::game::effects::token::{no_token, resolve_token_owner};
use crate::game::effects::token_copy::drive_copy_token_batches;
use crate::game::game_object::{DisplayIdentity, DisplaySource};
use crate::game::printed_cards::{
    copiable_values_from_face, printed_prepare_face, printed_ref_from_face,
};
use crate::game::quantity::resolve_quantity_with_targets;
use crate::types::ability::{Effect, EffectError, EffectKind, ResolvedAbility};
use crate::types::events::GameEvent;
use crate::types::game_state::{GameState, PendingCopyTokenBatch};
use crate::types::proposed_event::{CopyTokenSpec, TokenAttackRequest};
use std::collections::VecDeque;

pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let Effect::CreateTokenFromOracleCard { name, owner, count } = &ability.effect else {
        return Err(EffectError::MissingParam(
            "CreateTokenFromOracleCard".to_string(),
        ));
    };

    let Some(db) = state.card_db.clone() else {
        tracing::error!(
            "CreateTokenFromOracleCard resolved with no card database installed on \
             GameState; no token created. Every path that builds or restores a game \
             must call game::install_card_db."
        );
        return no_token(state, ability, events);
    };

    // CR 111.11: look up the Oracle card by name. CR 609.3: if no such card
    // exists in the loaded reference, do as much as possible (no token).
    let Some(face) = db.get_face_by_name(name).cloned() else {
        tracing::warn!(name = %name, "CreateTokenFromOracleCard: no Oracle card with this name");
        return no_token(state, ability, events);
    };
    let values = copiable_values_from_face(&face, printed_prepare_face(&db, &face));
    let printed_ref = printed_ref_from_face(&face);

    // CR 109.4 + CR 111.2: the creating player owns and controls the token.
    let owner_filter = owner.clone();
    let Some(token_owner) = resolve_token_owner(state, ability, &owner_filter) else {
        return no_token(state, ability, events);
    };
    let count = resolve_quantity_with_targets(state, count, ability).max(0) as u32;

    let mut remaining = VecDeque::with_capacity(1);
    remaining.push_back(PendingCopyTokenBatch {
        owner: token_owner,
        count,
        copy: Box::new(CopyTokenSpec {
            values: Box::new(values),
            display: DisplayIdentity {
                display_source: DisplaySource::Card,
                printed_ref,
                token_image_ref: None,
                token_art: None,
            },
            extra_keywords: Vec::new(),
            additional_modifications: Vec::new(),
            tapped: false,
            enters_attacking: false,
            attack_request: TokenAttackRequest::Unspecified,
            sacrifice_at: ability.duration.clone(),
            source_id: ability.source_id,
            controller: ability.controller,
            donor: None,
        }),
    });

    drive_copy_token_batches(
        state,
        remaining,
        EffectKind::from(&ability.effect),
        ability.source_id,
        events,
    );
    Ok(())
}
