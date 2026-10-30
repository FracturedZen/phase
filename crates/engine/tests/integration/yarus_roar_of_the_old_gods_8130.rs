//! Tests for issue #8130:
//! Yarus, Roar of the Old Gods — face-down creatures just die instead of returning and flipping face up.
//!
//! CR 603.10a: A trigger condition that checks for a permanent leaving the battlefield satisfies its condition
//! based on the characteristics and status of the object immediately prior to the event (LKI).
//! CR 708.2a / CR 708.4: Status includes face up/face down. A permanent's last known face-down status is
//! captured in its zone-change snapshot.
//! CR 708.7: "return it to the battlefield face down under its owner's control if it's a permanent card,
//! then turn it face up." When a face-down permanent card dies under its controller's control, Yarus's dies
//! trigger returns it to the battlefield face down and then turns it face up. Non-permanent cards (instants,
//! sorceries) remain in the graveyard.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::keywords::Keyword;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const YARUS: &str = "Other creatures you control have haste.\n\
     Whenever one or more face-down creatures you control deal combat damage to a player, draw a card.\n\
     Whenever a face-down creature you control dies, return it to the battlefield face down under its owner's control if it's a permanent card, then turn it face up.";

const DOOM_BLADE: &str = "Destroy target nonblack creature.";
const CYBER_CONVERSION: &str =
    "Turn target creature face down. It's a 2/2 Cyberman artifact creature.";
const SCROLL_OF_FATE: &str = "{T}: Manifest a card from your hand.";

/// A face-down creature permanent card you control dies; Yarus triggers, returning it face down
/// and then turning it face up on the battlefield.
#[test]
fn yarus_face_down_creature_dies_returns_face_down_and_turns_face_up() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let _yarus = scenario
        .add_creature_from_oracle(P0, "Yarus, Roar of the Old Gods", 4, 4, YARUS)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let cyber = scenario
        .add_spell_to_hand_from_oracle(P0, "Cyber Conversion", true, CYBER_CONVERSION)
        .id();
    let doom = scenario
        .add_spell_to_hand_from_oracle(P0, "Doom Blade", true, DOOM_BLADE)
        .id();

    let mut runner = scenario.build();

    // Turn Grizzly Bears face down
    runner.cast(cyber).target_object(bear).resolve();
    assert!(
        runner.state().objects[&bear].face_down,
        "Bear must be face down before destruction"
    );

    // Destroy the face-down Grizzly Bears
    let outcome = runner.cast(doom).target_object(bear).resolve();

    let bear_obj = outcome.state().objects.get(&bear).unwrap();
    assert_eq!(
        bear_obj.zone,
        Zone::Battlefield,
        "Bear must return to the battlefield"
    );
    assert!(
        !bear_obj.face_down,
        "Bear must be turned face up after returning"
    );
    assert_eq!(bear_obj.power, Some(2));
    assert_eq!(bear_obj.toughness, Some(2));
}

/// A face-up creature dies; Yarus must NOT trigger, and the creature remains in the graveyard.
#[test]
fn yarus_face_up_creature_dies_does_not_return() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let _yarus = scenario
        .add_creature_from_oracle(P0, "Yarus, Roar of the Old Gods", 4, 4, YARUS)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let doom = scenario
        .add_spell_to_hand_from_oracle(P0, "Doom Blade", true, DOOM_BLADE)
        .id();

    let mut runner = scenario.build();

    // Destroy face-up Grizzly Bears
    let outcome = runner.cast(doom).target_object(bear).resolve();

    let bear_obj = outcome.state().objects.get(&bear).unwrap();
    assert_eq!(
        bear_obj.zone,
        Zone::Graveyard,
        "Face-up creature must remain in the graveyard"
    );
}

/// A manifested instant card (not a permanent card) dies; Yarus's "if it's a permanent card"
/// condition fails, so it remains in the graveyard (CR 110.1).
#[test]
fn yarus_face_down_instant_dies_does_not_return() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let _yarus = scenario
        .add_creature_from_oracle(P0, "Yarus, Roar of the Old Gods", 4, 4, YARUS)
        .id();
    let scroll = scenario
        .add_artifact_from_oracle(P0, "Scroll of Fate", SCROLL_OF_FATE)
        .id();
    let shock = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, "Shock deals 2 damage to any target.")
        .id();
    let doom = scenario
        .add_spell_to_hand_from_oracle(P0, "Doom Blade", true, DOOM_BLADE)
        .id();

    let mut runner = scenario.build();

    // Activate Scroll of Fate to manifest Shock
    runner
        .act(GameAction::ActivateAbility {
            source_id: scroll,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("activate scroll");

    for _ in 0..16 {
        match &runner.state().waiting_for {
            WaitingFor::ChooseFromZoneChoice { .. } => break,
            _ => {
                runner.act(GameAction::PassPriority).unwrap();
            }
        }
    }

    runner
        .act(GameAction::SelectCards { cards: vec![shock] })
        .expect("select shock to manifest");

    assert!(
        runner.state().objects[&shock].face_down,
        "Shock must be manifested face down"
    );
    assert_eq!(runner.state().objects[&shock].zone, Zone::Battlefield);

    // Destroy manifested Shock
    let outcome = runner.cast(doom).target_object(shock).resolve();

    let shock_obj = outcome.state().objects.get(&shock).unwrap();
    assert_eq!(
        shock_obj.zone,
        Zone::Graveyard,
        "Manifested instant is not a permanent card in graveyard and must remain there"
    );
}

/// An opponent's face-down creature dies; Yarus triggers only for creatures you control,
/// so it does not trigger and remains in the opponent's graveyard.
#[test]
fn yarus_opponent_face_down_creature_dies_does_not_trigger() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let _yarus = scenario
        .add_creature_from_oracle(P0, "Yarus, Roar of the Old Gods", 4, 4, YARUS)
        .id();
    let opponent_bear = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();
    let cyber = scenario
        .add_spell_to_hand_from_oracle(P0, "Cyber Conversion", true, CYBER_CONVERSION)
        .id();
    let doom = scenario
        .add_spell_to_hand_from_oracle(P0, "Doom Blade", true, DOOM_BLADE)
        .id();

    let mut runner = scenario.build();

    // Turn opponent's bear face down
    runner.cast(cyber).target_object(opponent_bear).resolve();
    assert!(runner.state().objects[&opponent_bear].face_down);

    // Destroy opponent's face-down bear
    let outcome = runner.cast(doom).target_object(opponent_bear).resolve();

    let bear_obj = outcome.state().objects.get(&opponent_bear).unwrap();
    assert_eq!(
        bear_obj.zone,
        Zone::Graveyard,
        "Opponent's creature must remain in their graveyard"
    );
}

/// Yarus grants haste to other creatures you control ("Other creatures you control have haste.").
#[test]
fn yarus_grants_haste_to_other_creatures() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let yarus = scenario
        .add_creature_from_oracle(P0, "Yarus, Roar of the Old Gods", 4, 4, YARUS)
        .id();
    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();

    let mut runner = scenario.build();
    // Pass priority so layer modifications (layer 6: AddKeyword) are evaluated.
    runner.act(GameAction::PassPriority).ok();

    assert!(
        !runner.state().objects[&yarus]
            .keywords
            .contains(&Keyword::Haste),
        "Yarus does not grant haste to itself"
    );
    assert!(
        runner.state().objects[&bear]
            .keywords
            .contains(&Keyword::Haste),
        "Other creature you control gains haste from Yarus"
    );
}
