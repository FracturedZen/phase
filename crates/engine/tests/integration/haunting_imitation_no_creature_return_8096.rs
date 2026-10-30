//! Issue #8096 — Haunting Imitation: "Each player reveals the top card of
//! their library. For each creature card revealed this way, create a token
//! that's a copy of that card, except it's 1/1, it's a Spirit in addition to
//! its other types, and it has flying. If no creature cards were revealed this
//! way, return Haunting Imitation to its owner's hand."
//!
//! CR 608.2c: the "if no creature cards were revealed this way" sentence is a
//! condition on the return, read once after every player has revealed. It was
//! swallowed, so the return ran unconditionally, and with nothing revealed the
//! copy step copied the sorcery itself.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::CastPaymentMode;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const HAUNTING_IMITATION: &str = "Each player reveals the top card of their library. \
For each creature card revealed this way, create a token that's a copy of that card, \
except it's 1/1, it's a Spirit in addition to its other types, and it has flying. \
If no creature cards were revealed this way, return Haunting Imitation to its owner's hand.";

struct Setup {
    runner: GameRunner,
    sorcery: ObjectId,
}

/// `creature_tops[i]` decides whether player i's top card is a 3/3 creature.
fn setup(creature_tops: [bool; 2]) -> Setup {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let sorcery = scenario
        .add_spell_to_hand_from_oracle(P0, "Haunting Imitation", false, HAUNTING_IMITATION)
        .with_mana_cost(ManaCost::generic(3))
        .id();
    let mut tops = Vec::new();
    for (player, is_creature, name) in [
        (P0, creature_tops[0], "Mine"),
        (P1, creature_tops[1], "Theirs"),
    ] {
        scenario.with_library_top(player, &["Filler One", "Filler Two"]);
        let id = if is_creature {
            scenario
                .add_spell_to_library_top(player, &format!("{name} Beast"), false)
                .as_creature()
                .id()
        } else {
            scenario
                .add_spell_to_library_top(player, &format!("{name} Ritual"), false)
                .id()
        };
        tops.push((id, is_creature));
    }
    scenario.with_mana_pool(
        P0,
        (0..3)
            .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
            .collect(),
    );
    let mut runner = scenario.build();
    for (id, is_creature) in tops {
        if is_creature {
            let obj = runner.state_mut().objects.get_mut(&id).unwrap();
            obj.power = Some(3);
            obj.toughness = Some(3);
            obj.base_power = Some(3);
            obj.base_toughness = Some(3);
        }
    }
    Setup { runner, sorcery }
}

fn cast(setup: &mut Setup) {
    let card_id = setup.runner.state().objects[&setup.sorcery].card_id;
    setup
        .runner
        .act(GameAction::CastSpell {
            object_id: setup.sorcery,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast Haunting Imitation");
    setup.runner.advance_until_stack_empty();
}

fn tokens(runner: &GameRunner) -> Vec<ObjectId> {
    runner
        .state()
        .battlefield
        .iter()
        .copied()
        .filter(|id| runner.state().objects[id].is_token)
        .collect()
}

#[test]
fn no_creature_revealed_returns_haunting_imitation_to_hand() {
    let mut s = setup([false, false]);
    cast(&mut s);
    assert!(
        tokens(&s.runner).is_empty(),
        "no creature was revealed, so no token is created (got {:?})",
        tokens(&s.runner)
            .iter()
            .map(|id| s.runner.state().objects[id].name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        s.runner.state().objects[&s.sorcery].zone,
        Zone::Hand,
        "Haunting Imitation returns to its owner's hand"
    );
}

#[test]
fn one_creature_revealed_copies_it_and_does_not_return() {
    let mut s = setup([false, true]);
    cast(&mut s);
    let made = tokens(&s.runner);
    assert_eq!(made.len(), 1, "exactly one token for the one revealed creature");
    let token = &s.runner.state().objects[&made[0]];
    assert_eq!(token.name, "Theirs Beast");
    assert_eq!(token.controller, P0);
    assert_eq!((token.power, token.toughness), (Some(1), Some(1)));
    assert!(token.card_types.core_types.contains(&CoreType::Creature));
    assert!(token.card_types.subtypes.iter().any(|s| s == "Spirit"));
    assert!(token.has_keyword(&Keyword::Flying));
    assert_eq!(
        s.runner.state().objects[&s.sorcery].zone,
        Zone::Graveyard,
        "a creature was revealed, so Haunting Imitation is not returned"
    );
}

#[test]
fn every_revealed_creature_is_copied_once() {
    let mut s = setup([true, true]);
    cast(&mut s);
    let mut names: Vec<String> = tokens(&s.runner)
        .iter()
        .map(|id| s.runner.state().objects[id].name.clone())
        .collect();
    names.sort();
    assert_eq!(names, vec!["Mine Beast".to_string(), "Theirs Beast".to_string()]);
    assert_eq!(s.runner.state().objects[&s.sorcery].zone, Zone::Graveyard);
}
