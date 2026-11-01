//! Issue #10458: `ApplyPerpetual` must respect rejected `SelfRef` identity
//! (CR 400.7 / CR 400.7e) instead of falling back to raw `ability.source_id`.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::game::zones::move_to_zone;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const HOOK_HORROR_ORACLE: &str = "Molting Exoskeleton \u{2014} When Hook Horror dies, it perpetually gets -1/-1. Then if that card's toughness is 1 or greater, return it to the battlefield under its owner's control.";

fn build_hook_horror_runner() -> (GameRunner, ObjectId, [ObjectId; 3]) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let hook_id = scenario
        .add_creature_from_oracle(P0, "Hook Horror", 3, 3, HOOK_HORROR_ORACLE)
        .id();
    let murder_1 = scenario
        .add_spell_to_hand_from_oracle(P0, "Murder", true, "Destroy target creature.")
        .id();
    let murder_2 = scenario
        .add_spell_to_hand_from_oracle(P0, "Murder", true, "Destroy target creature.")
        .id();
    let murder_3 = scenario
        .add_spell_to_hand_from_oracle(P0, "Murder", true, "Destroy target creature.")
        .id();
    (scenario.build(), hook_id, [murder_1, murder_2, murder_3])
}

/// Commit `murder_id` targeting `target_id` and pass priority for both players
/// once so Murder resolves and puts Hook Horror's dies trigger on the stack
/// (stopping before the dies trigger itself resolves).
fn cast_and_resolve_murder_leaving_dies_trigger_on_stack(
    runner: &mut GameRunner,
    murder_id: ObjectId,
    target_id: ObjectId,
) {
    runner.cast(murder_id).target_object(target_id).commit();
    // Resolve Murder (top of stack); Hook Horror's dies trigger is placed on the stack.
    runner.pass_both_players();
    assert_eq!(
        runner.state().objects[&target_id].zone,
        Zone::Graveyard,
        "Hook Horror should be in the graveyard after Murder resolves"
    );
    assert_eq!(
        runner.state().stack.len(),
        1,
        "Hook Horror's dies trigger should be on the stack"
    );
}

/// CR 400.7e + CR 608.2c: When Hook Horror dies and remains the immediate
/// graveyard successor, its dies trigger gives it -1/-1 perpetually and returns
/// it to the battlefield while its toughness is 1 or greater; when its
/// toughness reaches 0 in the graveyard, it stays in the graveyard.
#[test]
fn hook_horror_immediate_graveyard_successor_gets_perpetual_debuff_and_returns_until_zero_toughness(
) {
    let (mut runner, hook_id, [murder_1, murder_2, murder_3]) = build_hook_horror_runner();

    // 1st death: 3/3 -> 2/2, returns to the battlefield.
    cast_and_resolve_murder_leaving_dies_trigger_on_stack(&mut runner, murder_1, hook_id);
    runner.pass_both_players();
    {
        let obj = &runner.state().objects[&hook_id];
        assert_eq!(obj.zone, Zone::Battlefield);
        assert_eq!((obj.base_power, obj.base_toughness), (Some(2), Some(2)));
        assert_eq!((obj.power, obj.toughness), (Some(2), Some(2)));
        assert_eq!(obj.perpetual_mods.len(), 1);
    }

    // 2nd death: 2/2 -> 1/1, returns to the battlefield.
    cast_and_resolve_murder_leaving_dies_trigger_on_stack(&mut runner, murder_2, hook_id);
    runner.pass_both_players();
    {
        let obj = &runner.state().objects[&hook_id];
        assert_eq!(obj.zone, Zone::Battlefield);
        assert_eq!((obj.base_power, obj.base_toughness), (Some(1), Some(1)));
        assert_eq!((obj.power, obj.toughness), (Some(1), Some(1)));
        assert_eq!(obj.perpetual_mods.len(), 2);
    }

    // 3rd death: 1/1 -> 0/0 in the graveyard; toughness is 0 (< 1), so it stays
    // in the graveyard.
    cast_and_resolve_murder_leaving_dies_trigger_on_stack(&mut runner, murder_3, hook_id);
    runner.pass_both_players();
    {
        let obj = &runner.state().objects[&hook_id];
        assert_eq!(
            obj.zone,
            Zone::Graveyard,
            "0-toughness Hook Horror must not return to the battlefield"
        );
        assert_eq!((obj.base_power, obj.base_toughness), (Some(0), Some(0)));
        assert_eq!((obj.power, obj.toughness), (Some(0), Some(0)));
        assert_eq!(obj.perpetual_mods.len(), 3);
    }
}

/// CR 400.7 + CR 400.7e: If Hook Horror dies and its graveyard card moves again
/// before the dies trigger resolves (even if it moves out of the graveyard and
/// back into the graveyard at the same `ObjectId`), the later incarnation is a
/// new object and must receive no perpetual modification and no battlefield
/// return when the stale dies trigger resolves.
#[test]
fn hook_horror_stale_incarnation_after_extra_zone_change_receives_no_perpetual_edit_or_return() {
    let (mut runner, hook_id, [murder_1, _, _]) = build_hook_horror_runner();

    cast_and_resolve_murder_leaving_dies_trigger_on_stack(&mut runner, murder_1, hook_id);

    // Move Hook Horror's graveyard card to Exile and back to Graveyard before
    // the dies trigger resolves, creating a later incarnation at the same
    // `ObjectId` in the graveyard.
    {
        let mut events = Vec::new();
        move_to_zone(runner.state_mut(), hook_id, Zone::Exile, &mut events);
        move_to_zone(runner.state_mut(), hook_id, Zone::Graveyard, &mut events);
    }
    let post_move_incarnation = runner.state().objects[&hook_id].incarnation;

    // Resolve the pending dies trigger.
    runner.pass_both_players();

    let obj = &runner.state().objects[&hook_id];
    assert_eq!(
        obj.zone,
        Zone::Graveyard,
        "stale dies trigger must not return a later graveyard incarnation to the battlefield"
    );
    assert_eq!(obj.incarnation, post_move_incarnation);
    assert!(
        obj.perpetual_mods.is_empty(),
        "later incarnation at the same ObjectId must receive no perpetual edit, got {:?}",
        obj.perpetual_mods
    );
    assert_eq!((obj.base_power, obj.base_toughness), (Some(3), Some(3)));
    assert_eq!((obj.power, obj.toughness), (Some(3), Some(3)));
}

/// CR 400.7 + CR 111.7: If Hook Horror is no longer present in `state.objects`
/// when its dies trigger resolves, the trigger resolves cleanly as a no-op.
#[test]
fn hook_horror_absent_object_when_dies_trigger_resolves_is_noop() {
    let (mut runner, hook_id, [murder_1, _, _]) = build_hook_horror_runner();

    cast_and_resolve_murder_leaving_dies_trigger_on_stack(&mut runner, murder_1, hook_id);

    runner.state_mut().objects.remove(&hook_id);
    for player in &mut runner.state_mut().players {
        player.graveyard.retain(|id| *id != hook_id);
    }

    // Resolve the pending dies trigger; must complete cleanly with no panic.
    runner.pass_both_players();
    assert!(runner.state().stack.is_empty());
    assert!(!runner.state().objects.contains_key(&hook_id));
}
