//! Issue #8132 — Winota, Joiner of Forces: "look at the top six cards of your
//! library. You may put a Human creature card from among them onto the
//! battlefield tapped and attacking. ... Put the rest of the cards on the
//! bottom of your library in a random order."
//!
//! CR 401.4 + CR 608.2c: every looked-at card that is not put onto the
//! battlefield goes to the bottom — whether no Human was among them, the
//! Human was declined, or the Human was taken.

use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const WINOTA_ORACLE: &str = "Whenever a non-Human creature you control attacks, look at the top six cards of your library. You may put a Human creature card from among them onto the battlefield tapped and attacking. It gains indestructible until end of turn. Put the rest of the cards on the bottom of your library in a random order.";

struct Setup {
    runner: GameRunner,
    attacker: ObjectId,
    /// The three cards below the top six, top-most first.
    deep: Vec<ObjectId>,
    /// The top six cards.
    seen: Vec<ObjectId>,
    human: Option<ObjectId>,
}

fn setup(with_human: bool) -> Setup {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let mut deep: Vec<ObjectId> = ["Deep 3", "Deep 2", "Deep 1"]
        .iter()
        .map(|n| scenario.add_card_to_library_top(P0, n))
        .collect();
    deep.reverse();
    let mut seen: Vec<ObjectId> = ["Seen 6", "Seen 5", "Seen 4", "Seen 3", "Seen 2", "Seen 1"]
        .iter()
        .map(|n| scenario.add_card_to_library_top(P0, n))
        .collect();
    seen.reverse();
    scenario
        .add_creature_from_oracle(P0, "Winota, Joiner of Forces", 4, 4, WINOTA_ORACLE)
        .with_subtypes(vec!["Human", "Warrior"]);
    let attacker = scenario
        .add_creature(P0, "Non-Human Attacker", 2, 2)
        .with_subtypes(vec!["Goblin"])
        .id();
    let mut runner = scenario.build();
    let human = with_human.then(|| seen[2]);
    if let Some(human) = human {
        let card = runner.state_mut().objects.get_mut(&human).unwrap();
        card.card_types.core_types = vec![CoreType::Creature];
        card.card_types.subtypes = vec!["Human".to_string()];
        card.base_card_types = card.card_types.clone();
        card.base_power = Some(1);
        card.base_toughness = Some(1);
        card.power = Some(1);
        card.toughness = Some(1);
    }
    Setup {
        runner,
        attacker,
        deep,
        seen,
        human,
    }
}

/// Attack with the non-Human and answer Winota's choice with `pick`.
fn attack(s: &mut Setup, pick: Option<ObjectId>) {
    s.runner.advance_to_combat();
    s.runner
        .declare_attackers(&[(s.attacker, AttackTarget::Player(P1))])
        .expect("declare the non-Human attacker");
    for _ in 0..60 {
        let result = match s.runner.state().waiting_for.clone() {
            WaitingFor::Priority { .. } if s.runner.state().stack.is_empty() => return,
            WaitingFor::Priority { .. } => s.runner.act(GameAction::PassPriority),
            WaitingFor::OrderTriggers { triggers, .. } => s.runner.act(GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            }),
            WaitingFor::DigChoice { .. } => s.runner.act(GameAction::SelectCards {
                cards: pick.into_iter().collect(),
            }),
            WaitingFor::EntryAttackTargetChoice { .. } => {
                s.runner.act(GameAction::ChooseEntryAttackTarget {
                    target: AttackTarget::Player(P1),
                })
            }
            other => panic!("unexpected prompt: {other:?}"),
        };
        result.expect("Winota's trigger accepts the scripted answer");
    }
    panic!("Winota's trigger never finished resolving");
}

fn library(s: &Setup) -> Vec<ObjectId> {
    s.runner.state().players[0].library.iter().copied().collect()
}

/// The untouched cards are now on top, and exactly `rest` fill the bottom.
fn assert_rest_on_bottom(s: &Setup, rest: &[ObjectId]) {
    let lib = library(s);
    assert_eq!(lib.len(), s.deep.len() + rest.len(), "library: {lib:?}");
    assert_eq!(
        &lib[..s.deep.len()],
        s.deep.as_slice(),
        "the unseen cards must now be on top: {lib:?}"
    );
    let mut bottom = lib[s.deep.len()..].to_vec();
    bottom.sort();
    let mut expected = rest.to_vec();
    expected.sort();
    assert_eq!(bottom, expected, "the rest must be on the bottom: {lib:?}");
}

#[test]
fn no_human_among_them_puts_all_six_on_the_bottom() {
    let mut s = setup(false);
    attack(&mut s, None);
    let seen = s.seen.clone();
    assert_rest_on_bottom(&s, &seen);
}

#[test]
fn declining_the_human_puts_all_six_on_the_bottom() {
    let mut s = setup(true);
    attack(&mut s, None);
    let seen = s.seen.clone();
    assert_rest_on_bottom(&s, &seen);
    assert_eq!(
        s.runner.state().objects[&s.human.unwrap()].zone,
        Zone::Library
    );
}

#[test]
fn taking_the_human_puts_the_other_five_on_the_bottom() {
    let mut s = setup(true);
    let human = s.human.unwrap();
    attack(&mut s, Some(human));
    assert_eq!(s.runner.state().objects[&human].zone, Zone::Battlefield);
    let rest: Vec<ObjectId> = s.seen.iter().copied().filter(|id| *id != human).collect();
    assert_rest_on_bottom(&s, &rest);
}
