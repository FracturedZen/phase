use crate::game::filter::{matches_target_filter, FilterContext};
use crate::game::targeting;
use crate::types::ability::{Effect, EffectError, EffectKind, ResolvedAbility, TargetFilter};
use crate::types::events::GameEvent;
use crate::types::game_state::{GameState, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::zones::Zone;

/// CR 609.7a: Choose a source of damage: a permanent, a spell on the stack, a
/// face-up command-zone object, or an object referred to by an object on the
/// stack, by a waiting replacement or prevention effect, or by a waiting delayed
/// triggered ability (even if it is no longer in the zone it used to be in).
pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let source_filter = match &ability.effect {
        Effect::ChooseDamageSource { source_filter } => source_filter.clone(),
        _ => {
            return Err(EffectError::InvalidParam(
                "expected ChooseDamageSource effect".to_string(),
            ))
        }
    };

    let options = damage_source_options(state, ability, &source_filter);
    state.waiting_for = WaitingFor::DamageSourceChoice {
        player: ability.controller,
        source_filter,
        options,
    };
    events.push(GameEvent::EffectResolved {
        kind: EffectKind::from(&ability.effect),
        source_id: ability.source_id,
        subject: None,
    });
    Ok(())
}

/// CR 609.7a: Enumerate the objects the controller may choose as a damage source
/// (battlefield, stack, command zone, and objects referred to by pending
/// objects/effects) matching `source_filter`. Shared with
/// `create_damage_replacement::resolve`, which prompts the same choice for an
/// inline "a source of your choice" one-shot damage replacement.
pub(crate) fn damage_source_options(
    state: &GameState,
    ability: &ResolvedAbility,
    source_filter: &TargetFilter,
) -> Vec<ObjectId> {
    let ctx = FilterContext::from_source_with_controller(ability.source_id, ability.controller);
    let mut options: Vec<ObjectId> = Vec::new();
    for id in [Zone::Battlefield, Zone::Stack]
        .into_iter()
        .flat_map(|zone| targeting::zone_object_ids(state, zone))
        .chain(state.command_zone.iter().copied())
        .chain(referred_by_pending_objects(state))
    {
        if !options.contains(&id)
            && state.objects.contains_key(&id)
            && matches_target_filter(state, id, source_filter, &ctx)
        {
            options.push(id);
        }
    }
    options
}

/// CR 609.7a: objects referred to by an object on the stack (its source and its
/// object targets), by a delayed triggered ability waiting to trigger (its
/// source and object targets), or by a replacement or prevention effect waiting
/// to apply (a specific damage source it names). Only these exact references
/// are offered; unrelated cards in other zones are not.
fn referred_by_pending_objects(state: &GameState) -> Vec<ObjectId> {
    let mut referred = Vec::new();
    for entry in state.stack.iter() {
        referred.push(entry.source_id);
        if let Some(ability) = entry.ability() {
            push_ability_object_targets(ability, &mut referred);
        }
    }
    for delayed in &state.delayed_triggers {
        referred.push(delayed.source_id);
        push_ability_object_targets(&delayed.ability, &mut referred);
    }
    let waiting_shields = state
        .pending_damage_replacements
        .iter()
        .chain(
            state
                .objects
                .values()
                .flat_map(|obj| obj.replacement_definitions.iter_unchecked())
                .filter(|def| def.is_resolution_installed()),
        )
        .filter(|def| !def.is_consumed);
    // The object map iterates in hash order; sort so the offered order is stable.
    let mut shield_sources: Vec<ObjectId> = waiting_shields
        .filter_map(|shield| match &shield.damage_source_filter {
            Some(TargetFilter::SpecificObject { id }) => Some(*id),
            _ => None,
        })
        .collect();
    shield_sources.sort_unstable();
    referred.extend(shield_sources);
    referred
}

/// Object targets of `ability` and its sub-ability chain.
fn push_ability_object_targets(ability: &ResolvedAbility, out: &mut Vec<ObjectId>) {
    let mut link = Some(ability);
    while let Some(current) = link {
        out.extend(targeting::object_targets(&current.targets));
        link = current.sub_ability.as_deref();
    }
}
