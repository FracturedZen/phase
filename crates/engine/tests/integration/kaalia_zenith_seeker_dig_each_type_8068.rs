//! Regression test for GitHub issue #8068 — Kaalia, Zenith Seeker only lets
//! you take an Angel card.
//!
//! Oracle (Kaalia, Zenith Seeker):
//! > Whenever Kaalia attacks, look at the top six cards of your library. You
//! > may reveal an Angel card, a Demon card, and/or a Dragon card from among
//! > them and put them into your hand. Put the rest on the bottom of your
//! > library in a random order.
//!
//! The bug: the from-among clause took only the first description, lowering to
//! `Dig { filter: Angel, keep_count: 1 }`, so Demons and Dragons were never
//! selectable. CR 701.20a + CR 608.2c: the list names up to one card per
//! description — up to three cards, each filling a distinct description, so two
//! Angels (or an Angel Demon counted twice) is not a legal keep.

use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{Effect, SearchSelectionConstraint, TargetFilter};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const KAALIA_ORACLE: &str = concat!(
    "Flying, trample, haste\n",
    "Whenever Kaalia, Zenith Seeker attacks, look at the top six cards of your ",
    "library. You may reveal an Angel card, a Demon card, and/or a Dragon card ",
    "from among them and put them into your hand. Put the rest on the bottom of ",
    "your library in a random order.",
);

/// Same Dig text on a loyalty ability so the runtime test drives the real
/// parser + resolver without a combat step.
const DIG_PLANESWALKER_ORACLE: &str = concat!(
    "+1: Look at the top six cards of your library. You may reveal an Angel ",
    "card, a Demon card, and/or a Dragon card from among them and put them into ",
    "your hand. Put the rest on the bottom of your library in a random order.",
);

#[test]
fn kaalia_attack_trigger_parses_one_card_per_description() {
    let parsed = parse_oracle_text(
        KAALIA_ORACLE,
        "Kaalia, Zenith Seeker",
        &[],
        &["Creature".into()],
        &["Human".into(), "Cleric".into()],
    );
    let trigger = parsed.triggers.first().expect("attack trigger parsed");
    let execute = trigger.execute.as_ref().expect("trigger has an effect");
    let Effect::Dig {
        keep_count,
        up_to,
        filter,
        destination,
        reveal,
        selection_constraint,
        ..
    } = &*execute.effect
    else {
        panic!("expected Dig, got {:?}", execute.effect);
    };
    assert_eq!(*keep_count, Some(3), "up to one card for each of three descriptions");
    assert!(*up_to, "each description is optional ('you may', 'and/or')");
    assert!(*reveal);
    assert_eq!(*destination, Some(Zone::Hand));
    let TargetFilter::Or { filters } = filter else {
        panic!("expected Or filter over the three descriptions, got {filter:?}");
    };
    assert_eq!(filters.len(), 3);
    assert_eq!(
        *selection_constraint,
        SearchSelectionConstraint::MatchEachFilter {
            filters: filters.clone()
        },
        "each kept card must fill a distinct description"
    );
}

/// The ordinary single-description article form keeps its old lowering.
#[test]
fn single_description_from_among_is_unconstrained() {
    let parsed = parse_oracle_text(
        "Whenever Test Seeker attacks, look at the top six cards of your library. \
         You may reveal an Angel card from among them and put it into your hand. \
         Put the rest on the bottom of your library in a random order.",
        "Test Seeker",
        &[],
        &["Creature".into()],
        &[],
    );
    let execute = parsed.triggers[0].execute.as_ref().unwrap();
    let Effect::Dig {
        keep_count,
        selection_constraint,
        ..
    } = &*execute.effect
    else {
        panic!("expected Dig");
    };
    assert_eq!(*keep_count, Some(1));
    assert_eq!(*selection_constraint, SearchSelectionConstraint::None);
}

struct Setup {
    runner: GameRunner,
    angel: ObjectId,
    angel_two: ObjectId,
    demon: ObjectId,
    dragon: ObjectId,
    others: Vec<ObjectId>,
}

fn setup() -> Setup {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let walker = scenario
        .add_planeswalker_from_oracle(P0, "Seeker Walker", "Seeker", 3, DIG_PLANESWALKER_ORACLE)
        .id();
    let subtypes = ["Angel", "Angel", "Demon", "Dragon", "Human", "Elf"];
    let mut ids = vec![ObjectId(0); 6];
    for i in (0..6).rev() {
        ids[i] = scenario.add_card_to_library_top(P0, &format!("Library Card {i}"));
    }
    let mut runner = scenario.build();
    for (i, &id) in ids.iter().enumerate() {
        let obj = runner.state_mut().objects.get_mut(&id).unwrap();
        obj.card_types.core_types = vec![CoreType::Creature];
        obj.card_types.subtypes = vec![subtypes[i].to_string()];
    }
    {
        let state = runner.state_mut();
        state.active_player = P0;
        state.priority_player = P0;
        state.layers_dirty.mark_full();
    }
    evaluate_layers(runner.state_mut());
    runner.activate(walker, 0).resolve();
    Setup {
        runner,
        angel: ids[0],
        angel_two: ids[1],
        demon: ids[2],
        dragon: ids[3],
        others: vec![ids[4], ids[5]],
    }
}

#[test]
fn kaalia_dig_offers_and_keeps_an_angel_a_demon_and_a_dragon() {
    let mut s = setup();
    let WaitingFor::DigChoice {
        selectable_cards,
        keep_count,
        ..
    } = &s.runner.state().waiting_for
    else {
        panic!("expected DigChoice, got {}", s.runner.waiting_for_kind());
    };
    assert_eq!(*keep_count, 3);
    for id in [s.angel, s.angel_two, s.demon, s.dragon] {
        assert!(selectable_cards.contains(&id), "{id:?} must be selectable");
    }
    for id in &s.others {
        assert!(!selectable_cards.contains(id), "non-matching card selectable");
    }

    s.runner
        .act(GameAction::SelectCards {
            cards: vec![s.angel, s.demon, s.dragon],
        })
        .expect("an Angel, a Demon and a Dragon is a legal keep");
    let state = s.runner.state();
    for id in [s.angel, s.demon, s.dragon] {
        assert_eq!(state.objects[&id].zone, Zone::Hand);
    }
    for id in [s.angel_two, s.others[0], s.others[1]] {
        assert_eq!(state.objects[&id].zone, Zone::Library);
    }
}

#[test]
fn kaalia_dig_rejects_two_cards_for_the_same_description() {
    let mut s = setup();
    let err = s
        .runner
        .act(GameAction::SelectCards {
            cards: vec![s.angel, s.angel_two],
        })
        .expect_err("two Angels fill only one description");
    assert!(
        format!("{err:?}").contains("selection constraint"),
        "unexpected error {err:?}"
    );
    assert!(matches!(
        s.runner.state().waiting_for,
        WaitingFor::DigChoice { .. }
    ));

    // A partial keep (just a Dragon) stays legal.
    s.runner
        .act(GameAction::SelectCards {
            cards: vec![s.dragon],
        })
        .expect("a lone Dragon is a legal keep");
    assert_eq!(s.runner.state().objects[&s.dragon].zone, Zone::Hand);
}
