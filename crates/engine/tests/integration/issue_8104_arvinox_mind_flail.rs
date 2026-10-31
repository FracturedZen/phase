//! Regression for issue #8104: Arvinox, the Mind Flail.
//!
//! * CR 205.1a + CR 613.1d + CR 611.3a: "Arvinox isn't a creature unless you
//!   control three or more permanents you don't own" is a self type-removal
//!   static that applies exactly while the condition is false.
//! * CR 401.2 + CR 406.3: at your end step it exiles the BOTTOM card of each
//!   opponent's library face down.
//! * CR 601.2a + CR 609.4b + CR 611.2a: for as long as those cards remain
//!   exiled, Arvinox's controller may look at them and cast permanent spells
//!   from among them, spending mana as though it were mana of any color.
//!
//! https://github.com/phase-rs/phase/issues/8104

use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    CastingPermission, ContinuousModification, Effect, LibraryPosition, ManaSpendPermission,
    StaticCondition, TargetFilter, TypeFilter,
};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const NAME: &str = "Arvinox, the Mind Flail";
const ORACLE: &str = "Arvinox isn't a creature unless you control three or more permanents you don't own.\n\
At the beginning of your end step, exile the bottom card of each opponent's library face down. \
For as long as those cards remain exiled, you may look at them, you may cast permanent spells from among them, \
and you may spend mana as though it were mana of any color to cast those spells.";

const P2: PlayerId = PlayerId(2);

fn is_creature(runner: &GameRunner, id: ObjectId) -> bool {
    runner.state().objects[&id]
        .card_types
        .core_types
        .contains(&CoreType::Creature)
}

#[test]
fn arvinox_parses_static_exile_bottom_and_typed_cast_grant() {
    let parsed = parse_oracle_text(
        ORACLE,
        NAME,
        &[],
        &[
            "Legendary".to_string(),
            "Enchantment".to_string(),
            "Creature".to_string(),
        ],
        &["Horror".to_string()],
    );

    // Line 1: a static, not a spell ability.
    assert!(parsed.abilities.is_empty(), "{:#?}", parsed.abilities);
    assert_eq!(parsed.statics.len(), 1, "{:#?}", parsed.statics);
    let def = &parsed.statics[0];
    assert_eq!(def.affected, Some(TargetFilter::SelfRef));
    assert!(matches!(
        def.modifications.as_slice(),
        [ContinuousModification::RemoveType {
            core_type: CoreType::Creature
        }]
    ));
    assert!(
        matches!(def.condition, Some(StaticCondition::Not { .. })),
        "the unless rider negates the gate: {:?}",
        def.condition
    );

    // Line 2: end-step trigger → bottom-edge face-down exile + typed grant.
    assert_eq!(parsed.triggers.len(), 1);
    let execute = parsed.triggers[0].execute.as_ref().expect("trigger body");
    let json = serde_json::to_string(execute).unwrap();
    assert!(!json.contains("Unimplemented"), "{json}");
    match &*execute.effect {
        Effect::ExileTop {
            position,
            face_down,
            ..
        } => {
            assert_eq!(*position, LibraryPosition::Bottom, "exiles the BOTTOM card");
            assert!(*face_down);
        }
        other => panic!("expected ExileTop, got {other:?}"),
    }
    let grant = execute.sub_ability.as_ref().expect("cast grant");
    match &*grant.effect {
        Effect::GrantCastingPermission {
            permission:
                CastingPermission::PlayFromExile {
                    card_filter,
                    mana_spend_permission,
                    single_use,
                    grants_look,
                    ..
                },
            ..
        } => {
            assert!(*grants_look, "\"you may look at them\" rides on the grant");
            assert_eq!(*mana_spend_permission, Some(ManaSpendPermission::AnyColor));
            assert!(!*single_use, "permanent spells — not a single cast");
            let filter = card_filter.as_ref().expect("permanent filter");
            assert!(
                matches!(filter, TargetFilter::Typed(t) if t.type_filters == vec![TypeFilter::Permanent]),
                "{filter:?}"
            );
        }
        other => panic!("expected GrantCastingPermission, got {other:?}"),
    }
}

fn arvinox_with_stolen(count: usize) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let arvinox = scenario
        .add_creature_from_oracle(P0, NAME, 2, 5, ORACLE)
        .id();
    for i in 0..count {
        scenario
            .add_creature(P1, &format!("Stolen {i}"), 1, 1)
            .controlled_by(P0);
    }
    // Permanents P0 owns do not count.
    scenario.add_creature(P0, "Own Bear", 2, 2);
    scenario.add_creature(P0, "Own Elk", 3, 3);
    scenario.add_creature(P0, "Own Wolf", 2, 2);
    let mut runner = scenario.build();
    runner.state_mut().layers_dirty.mark_full();
    evaluate_layers(runner.state_mut());
    (runner, arvinox)
}

#[test]
fn arvinox_is_not_a_creature_with_fewer_than_three_unowned_permanents() {
    let (runner, arvinox) = arvinox_with_stolen(2);
    assert!(
        !is_creature(&runner, arvinox),
        "two permanents you don't own: Arvinox isn't a creature"
    );
}

#[test]
fn arvinox_is_a_creature_with_three_unowned_permanents() {
    let (runner, arvinox) = arvinox_with_stolen(3);
    assert!(
        is_creature(&runner, arvinox),
        "three permanents you don't own: Arvinox is a creature"
    );
}

fn reach_end_step(runner: &mut GameRunner) {
    runner.advance_to_end_step();
    for _ in 0..32 {
        match runner.state().waiting_for.clone() {
            WaitingFor::DeclareAttackers { .. } => {
                runner
                    .act(GameAction::DeclareAttackers {
                        attacks: vec![],
                        bands: vec![],
                    })
                    .expect("empty attack declaration");
            }
            _ if runner.state().phase == Phase::End => return,
            _ => runner.pass_both_players(),
        }
    }
    panic!("never reached the end step");
}

#[test]
fn arvinox_exiles_bottom_card_of_each_opponent_face_down_with_cast_grant() {
    let mut scenario = GameScenario::new_n_player(3, 8104);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, NAME, 2, 5, ORACLE);
    let mut bottoms = Vec::new();
    let mut tops = Vec::new();
    for opp in [P1, P2] {
        // Added first → ends up at the bottom.
        bottoms.push(scenario.add_card_to_library_top(opp, "Bottom Card"));
        tops.push(scenario.add_card_to_library_top(opp, "Top Card"));
    }
    scenario.add_card_to_library_top(P0, "Own Card");
    let mut runner = scenario.build();

    reach_end_step(&mut runner);
    runner.advance_until_stack_empty();

    for &bottom in &bottoms {
        let obj = &runner.state().objects[&bottom];
        assert_eq!(obj.zone, Zone::Exile, "the bottom card is exiled");
        assert!(obj.face_down, "exiled face down");
        let grant = obj
            .casting_permissions
            .iter()
            .find_map(|p| match p {
                CastingPermission::PlayFromExile {
                    granted_to,
                    card_filter,
                    mana_spend_permission,
                    ..
                } => Some((*granted_to, card_filter.clone(), *mana_spend_permission)),
                _ => None,
            })
            .expect("the exiled card carries Arvinox's cast permission");
        assert_eq!(
            grant.0, P0,
            "granted to Arvinox's controller, not the opponent"
        );
        assert!(grant.1.is_some(), "permanent spells only");
        assert_eq!(grant.2, Some(ManaSpendPermission::AnyColor));
    }
    for &top in &tops {
        assert_eq!(
            runner.state().objects[&top].zone,
            Zone::Library,
            "the top card stays in the library"
        );
    }
}

/// CR 406.3: "you may look at them" covers EVERY card exiled this way — an
/// instant Arvinox's controller can't cast is still visible to them, while a
/// player without the grant (another opponent) still can't see it.
#[test]
fn arvinox_controller_may_look_at_exiled_instant_it_cannot_cast() {
    let mut scenario = GameScenario::new_n_player(3, 8104);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, NAME, 2, 5, ORACLE);
    // Added first → the bottom card of P1's library.
    let instant = scenario
        .add_spell_to_library_top(P1, "Secret Bolt", true)
        .id();
    scenario.add_card_to_library_top(P1, "P1 Top");
    let other = scenario.add_card_to_library_top(P2, "Other Secret");
    scenario.add_card_to_library_top(P2, "P2 Top");
    let mut runner = scenario.build();

    reach_end_step(&mut runner);
    runner.advance_until_stack_empty();

    let state = runner.state();
    for id in [instant, other] {
        assert_eq!(state.objects[&id].zone, Zone::Exile);
        assert!(state.objects[&id].face_down);
    }
    let seen = |viewer: PlayerId, id: ObjectId| {
        engine::game::visibility::filter_state_for_viewer(state, viewer).objects[&id]
            .name
            .clone()
    };
    assert_eq!(
        seen(P0, instant),
        "Secret Bolt",
        "Arvinox's controller may look at the exiled instant"
    );
    assert_eq!(seen(P0, other), "Other Secret");
    assert_ne!(
        seen(P2, instant),
        "Secret Bolt",
        "an opponent without the grant can't look at P1's face-down card"
    );
    // The look right is not cast authority: the instant fails the permanent
    // filter, so the only grant on it never admits a cast.
    let grant = state.objects[&instant]
        .casting_permissions
        .iter()
        .find_map(|p| match p {
            CastingPermission::PlayFromExile { card_filter, .. } => card_filter.clone(),
            _ => None,
        })
        .expect("typed grant");
    assert!(
        matches!(grant, TargetFilter::Typed(t) if t.type_filters == vec![TypeFilter::Permanent])
    );
}
