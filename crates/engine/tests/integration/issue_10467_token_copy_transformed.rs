//! Issue #10467: Token copy of a back-face-up double-faced permanent enters
//! front-face flagged (CR 707.8a; Cackling Counterpart on transformed Thraben Gargoyle).

use engine::game::game_object::BackFaceData;
use engine::game::scenario::{GameScenario, P0};
use engine::game::transform::transform_permanent;
use engine::types::actions::GameAction;
use engine::types::card_type::{CardType, CoreType};
use engine::types::events::GameEvent;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;

const THRABEN_GARGOYLE_ORACLE: &str = "Defender\n{6}: Transform this creature.";
const CACKLING_COUNTERPART_ORACLE: &str = "Create a token that's a copy of target creature you control.\nFlashback {5}{U}{U} (You may cast this card from your graveyard for its flashback cost. Then exile it.)";

fn stonewing_antagonizer_back_face() -> BackFaceData {
    BackFaceData {
        is_swap_snapshot: false,
        trigger_printed_origins: Vec::new(),
        name: "Stonewing Antagonizer".to_string(),
        power: Some(4),
        toughness: Some(2),
        loyalty: None,
        printed_loyalty: None,
        defense: None,
        card_types: CardType {
            supertypes: vec![],
            core_types: vec![CoreType::Creature],
            subtypes: vec!["Gargoyle".to_string(), "Horror".to_string()],
        },
        mana_cost: ManaCost::default(),
        keywords: vec![Keyword::Flying],
        abilities: vec![],
        trigger_definitions: Default::default(),
        replacement_definitions: Default::default(),
        static_definitions: Default::default(),
        color: vec![],
        printed_ref: None,
        modal: None,
        additional_cost: None,
        strive_cost: None,
        casting_restrictions: vec![],
        casting_options: vec![],
        layout_kind: None,
        parse_warnings: vec![],
    }
}

#[test]
fn token_copy_of_back_face_up_dfc_enters_back_face_up_and_can_transform() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let gargoyle = scenario
        .add_creature(P0, "Thraben Gargoyle", 2, 2)
        .from_oracle_text(THRABEN_GARGOYLE_ORACLE)
        .id();
    let counterpart = scenario
        .add_spell_to_hand_from_oracle(
            P0,
            "Cackling Counterpart",
            true,
            CACKLING_COUNTERPART_ORACLE,
        )
        .with_mana_cost(ManaCost::zero())
        .id();

    // {6} for Gargoyle activation
    scenario.with_mana_pool(
        P0,
        (0..6)
            .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
            .collect(),
    );

    let mut runner = scenario.build();
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };

    runner
        .state_mut()
        .objects
        .get_mut(&gargoyle)
        .unwrap()
        .back_face = Some(stonewing_antagonizer_back_face());

    // 1. Activate {6}: Transform Thraben Gargoyle.
    let ability_idx = runner.state().objects[&gargoyle]
        .abilities
        .iter()
        .position(|a| {
            matches!(
                a.effect.as_ref(),
                engine::types::ability::Effect::Transform { .. }
            )
        })
        .expect("Thraben Gargoyle transform ability");

    runner
        .act(GameAction::ActivateAbility {
            source_id: gargoyle,
            ability_index: ability_idx,
            payment_mode: engine::types::game_state::CastPaymentMode::Auto,
        })
        .expect("activate transform");

    // Pass priority so the transform ability resolves.
    let mut transform_events = Vec::new();
    while !runner.state().stack.is_empty() {
        let outcome = runner.act(GameAction::PassPriority).expect("pass priority");
        transform_events.extend(outcome.events);
    }

    assert!(
        runner.state().objects[&gargoyle].transformed,
        "Thraben Gargoyle must be transformed"
    );
    assert_eq!(
        runner.state().objects[&gargoyle].name,
        "Stonewing Antagonizer"
    );

    // 2. Cast Cackling Counterpart targeting the transformed creature (Stonewing Antagonizer).
    let cast_outcome = runner
        .cast(counterpart)
        .target_objects(&[gargoyle])
        .resolve();

    let events = cast_outcome.events();
    let state = cast_outcome.state();

    // Verify token entry does NOT emit GameEvent::Transformed (CR 712.14a).
    let transform_events_during_copy: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, GameEvent::Transformed { .. }))
        .collect();
    assert!(
        transform_events_during_copy.is_empty(),
        "CR 712.14a: entering back face up is not a transform event: {transform_events_during_copy:#?}"
    );

    // Find the newly created token on the battlefield.
    let token_id = state
        .battlefield
        .iter()
        .copied()
        .find(|&id| id != gargoyle && state.objects[&id].is_token)
        .expect("a token copy must be created on the battlefield");

    let token = &state.objects[&token_id];

    // CR 707.8a: The token enters the battlefield with its back face up.
    assert!(
        token.transformed,
        "CR 707.8a: token copy of back-face-up DFC must enter with transformed == true"
    );
    assert_eq!(
        token.name, "Stonewing Antagonizer",
        "token must have the back-face name"
    );
    assert_eq!(token.power, Some(4));
    assert_eq!(token.toughness, Some(2));
    assert!(
        token.keywords.contains(&Keyword::Flying),
        "token must have Flying from back face"
    );

    // CR 707.8 / CR 701.27a: The token keeps both faces in back_face.
    assert!(
        token.back_face.is_some(),
        "CR 707.8: token must have a back_face representing the front face"
    );
    let front_face = token.back_face.as_ref().unwrap();
    assert_eq!(
        front_face.name, "Thraben Gargoyle",
        "stashed front face must be Thraben Gargoyle"
    );

    // 3. A later transform turns it front face up (CR 701.27a).
    let mut manual_transform_events = Vec::new();
    transform_permanent(runner.state_mut(), token_id, &mut manual_transform_events)
        .expect("transforming the token must succeed");

    let transformed_token = &runner.state().objects[&token_id];
    assert!(
        !transformed_token.transformed,
        "CR 701.27a: transforming back-face-up token turns it front face up (transformed == false)"
    );
    assert_eq!(
        transformed_token.name, "Thraben Gargoyle",
        "transformed token is now front face Thraben Gargoyle"
    );
    assert_eq!(transformed_token.power, Some(2));
    assert_eq!(transformed_token.toughness, Some(2));
    assert!(
        transformed_token.keywords.contains(&Keyword::Defender),
        "front face has Defender"
    );
}
