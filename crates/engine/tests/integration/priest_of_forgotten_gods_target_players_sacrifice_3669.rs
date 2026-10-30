//! Regression for GitHub issue #3669 — Priest of Forgotten Gods.
//!
//! "{T}, Sacrifice two other creatures: Any number of target players each
//! lose 2 life and sacrifice a creature of their choice. You add {B}{B} and
//! draw a card."
//!
//! The conjoined "sacrifice a creature of their choice" shares the "each
//! target player" subject (CR 115.1d + CR 608.2c), so each chosen player
//! sacrifices a creature they choose (CR 701.21a), in APNAP order (CR 101.4).
//! The bug lowered the conjunct as a subjectless imperative whose filter had
//! no controller, so the Priest's controller sacrificed instead and the
//! targeted players kept their creatures. "You add {B}{B} and draw a card"
//! is a new sentence and still happens once, for the controller.

use engine::game::scenario::{GameRunner, GameScenario};
use engine::types::ability::{ControllerRef, Effect, TargetFilter, TargetRef};
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaType;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const PRIEST: &str = "{T}, Sacrifice two other creatures: Any number of target players each \
     lose 2 life and sacrifice a creature of their choice. You add {B}{B} and draw a card.";

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

struct Board {
    runner: GameRunner,
    priest: ObjectId,
    fodder: [ObjectId; 2],
    keeper: ObjectId,
    p1_bear: ObjectId,
    p2_bear: ObjectId,
}

fn setup() -> Board {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Library Card A", "Library Card B"]);
    let priest = scenario
        .add_creature(P0, "Priest of Forgotten Gods", 1, 2)
        .from_oracle_text(PRIEST)
        .id();
    let f1 = scenario.add_creature(P0, "Fodder One", 1, 1).id();
    let f2 = scenario.add_creature(P0, "Fodder Two", 1, 1).id();
    let keeper = scenario.add_creature(P0, "Keeper", 2, 2).id();
    let p1_bear = scenario.add_creature(P1, "P1 Bear", 2, 2).id();
    let p2_bear = scenario.add_creature(P2, "P2 Bear", 2, 2).id();
    Board {
        runner: scenario.build(),
        priest,
        fodder: [f1, f2],
        keeper,
        p1_bear,
        p2_bear,
    }
}

fn zone(runner: &GameRunner, id: ObjectId) -> Zone {
    runner.state().objects.get(&id).expect("object exists").zone
}

fn life(runner: &GameRunner, p: PlayerId) -> i32 {
    runner.state().players.iter().find(|x| x.id == p).unwrap().life
}

/// Activate the Priest targeting `targets`, pay the sacrifice cost with the
/// two fodder creatures, and resolve. Returns every player that was asked to
/// make the effect's sacrifice choice.
fn activate_and_resolve(board: &mut Board, targets: Vec<PlayerId>) -> Vec<PlayerId> {
    let runner = &mut board.runner;
    runner
        .act(GameAction::ActivateAbility {
            source_id: board.priest,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("activate Priest");
    let mut choosers = Vec::new();
    for _ in 0..40 {
        let waiting = runner.state().waiting_for.clone();
        match waiting {
            WaitingFor::TargetSelection { .. } | WaitingFor::MultiTargetSelection { .. } => {
                runner
                    .act(GameAction::SelectTargets {
                        targets: targets.iter().map(|&p| TargetRef::Player(p)).collect(),
                    })
                    .expect("select player targets");
            }
            WaitingFor::PayCost { .. } => {
                runner
                    .act(GameAction::SelectCards {
                        cards: board.fodder.to_vec(),
                    })
                    .expect("pay sacrifice cost");
            }
            WaitingFor::EffectZoneChoice { player, cards, .. } => {
                choosers.push(player);
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![cards[0]],
                    })
                    .expect("choose sacrifice");
            }
            WaitingFor::Priority { .. } => {
                if runner.state().stack.is_empty() {
                    return choosers;
                }
                runner
                    .act(GameAction::PassPriority)
                    .expect("pass priority");
            }
            other => panic!("unexpected prompt: {other:?}"),
        }
    }
    panic!("activation did not finish resolving");
}

#[test]
fn compound_sacrifice_binds_to_each_target_player() {
    let def = engine::parser::oracle_effect::parse_effect_chain(
        "Any number of target players each lose 2 life and sacrifice a creature of their choice. \
         You add {B}{B} and draw a card.",
        engine::types::ability::AbilityKind::Activated,
    );
    assert!(def.multi_target.is_some());
    let sac = def.sub_ability.as_deref().expect("sacrifice conjunct");
    match sac.effect.as_ref() {
        Effect::Sacrifice {
            target: TargetFilter::Typed(tf),
            ..
        } => assert_eq!(
            tf.controller,
            Some(ControllerRef::ParentTargetController),
            "the sacrifice belongs to each targeted player"
        ),
        other => panic!("expected Sacrifice conjunct, got {other:?}"),
    }
}

#[test]
fn each_target_opponent_loses_life_and_sacrifices_their_creature() {
    let mut board = setup();
    let hand_before = board.runner.state().players[0].hand.len();
    let choosers = activate_and_resolve(&mut board, vec![P1, P2]);
    let runner = &board.runner;

    assert!(!choosers.contains(&P0), "controller must not sacrifice: {choosers:?}");
    assert_eq!(zone(runner, board.p1_bear), Zone::Graveyard, "P1 sacrifices");
    assert_eq!(zone(runner, board.p2_bear), Zone::Graveyard, "P2 sacrifices");
    assert_eq!(zone(runner, board.keeper), Zone::Battlefield, "P0 keeps Keeper");
    assert_eq!(zone(runner, board.priest), Zone::Battlefield);
    assert_eq!([life(runner, P0), life(runner, P1), life(runner, P2)], [20, 18, 18]);

    let p0 = &runner.state().players[0];
    assert_eq!(p0.mana_pool.count_color(ManaType::Black), 2, "You add {{B}}{{B}} once");
    assert_eq!(p0.hand.len(), hand_before + 1, "controller draws exactly one card");
}

#[test]
fn single_target_player_sacrifices_and_others_are_untouched() {
    let mut board = setup();
    let choosers = activate_and_resolve(&mut board, vec![P2]);
    let runner = &board.runner;

    assert!(!choosers.contains(&P0), "controller must not sacrifice: {choosers:?}");
    assert_eq!(zone(runner, board.p2_bear), Zone::Graveyard, "P2 sacrifices");
    assert_eq!(zone(runner, board.p1_bear), Zone::Battlefield, "P1 was not targeted");
    assert_eq!(zone(runner, board.keeper), Zone::Battlefield);
    assert_eq!([life(runner, P0), life(runner, P1), life(runner, P2)], [20, 20, 18]);
}

#[test]
fn targeting_self_and_opponent_each_sacrifice_their_own() {
    let mut board = setup();
    let choosers = activate_and_resolve(&mut board, vec![P0, P1]);
    let runner = &board.runner;

    // P0's only other creature is Keeper (the fodder was sacrificed as cost),
    // and the Priest itself is a creature: P0 sacrifices exactly one of them.
    let p0_left = [board.keeper, board.priest]
        .iter()
        .filter(|&&id| zone(runner, id) == Zone::Battlefield)
        .count();
    assert_eq!(p0_left, 1, "P0 sacrifices exactly one creature ({choosers:?})");
    assert_eq!(zone(runner, board.p1_bear), Zone::Graveyard, "P1 sacrifices");
    assert_eq!(zone(runner, board.p2_bear), Zone::Battlefield);
    assert_eq!([life(runner, P0), life(runner, P1), life(runner, P2)], [18, 18, 20]);
}
