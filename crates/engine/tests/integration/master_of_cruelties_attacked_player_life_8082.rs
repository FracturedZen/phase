//! Issue #8082 — Master of Cruelties: "Whenever this creature attacks a player
//! and isn't blocked, that player's life total becomes 1."
//!
//! CR 509.3c: an "attacks and isn't blocked" ability triggers once no blockers
//! are declared for the creature, even when it was put onto the battlefield
//! attacking (Kaalia of the Vast). CR 506.2 + CR 508.5 + CR 608.2c: "that
//! player" is the attacked (defending) player, never the controller. The
//! trigger also requires the attacked object to be a player, so an unblocked
//! attack on a planeswalker does not trigger.
//!
//! Before the fix "that player's life total" parsed as an untargeted
//! `ParentTarget`, which resolved to the controller: the attacking player's
//! life became 1 and the attacked player's life was untouched.

use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::TargetFilter;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const P2: PlayerId = PlayerId(2);

const MASTER_OF_CRUELTIES: &str = "First strike, deathtouch\n\
This creature can only attack alone.\n\
Whenever this creature attacks a player and isn't blocked, that player's life total becomes 1. This creature assigns no combat damage this combat.";

const KAALIA: &str = "Flying\nWhenever Kaalia attacks an opponent, you may put an Angel, Demon, or Dragon creature card from your hand onto the battlefield tapped and attacking that opponent.";

/// Drive combat with no blocks, accepting optional effects, choosing `pick`
/// for any zone choice and `defender` for an enters-attacking choice, until
/// combat is over.
fn run_combat(runner: &mut GameRunner, pick: ObjectId, defender: PlayerId) {
    for _ in 0..200 {
        let action = match runner.state().waiting_for.clone() {
            WaitingFor::Priority { .. } => GameAction::PassPriority,
            WaitingFor::DeclareBlockers { .. } => GameAction::DeclareBlockers {
                assignments: vec![],
            },
            WaitingFor::OptionalEffectChoice { .. } => {
                GameAction::DecideOptionalEffect { accept: true }
            }
            WaitingFor::ChooseFromZoneChoice { .. } | WaitingFor::EffectZoneChoice { .. } => {
                GameAction::SelectCards { cards: vec![pick] }
            }
            WaitingFor::EntryAttackTargetChoice { .. } => GameAction::ChooseEntryAttackTarget {
                target: AttackTarget::Player(defender),
            },
            WaitingFor::OrderTriggers { triggers, .. } => GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            },
            other => panic!("unexpected prompt during combat: {other:?}"),
        };
        runner.act(action).expect("combat action");
        if matches!(
            runner.state().phase,
            Phase::EndCombat | Phase::PostCombatMain
        ) {
            return;
        }
    }
    panic!("combat did not finish");
}

fn life(runner: &GameRunner, player: PlayerId) -> i32 {
    runner
        .state()
        .players
        .iter()
        .find(|p| p.id == player)
        .expect("player")
        .life
}

fn add_master(scenario: &mut GameScenario) -> ObjectId {
    scenario
        .add_creature_from_oracle(P0, "Master of Cruelties", 1, 4, MASTER_OF_CRUELTIES)
        .with_keyword(Keyword::FirstStrike)
        .with_keyword(Keyword::Deathtouch)
        .id()
}

#[test]
fn unblocked_trigger_binds_that_player_to_defending_player() {
    let parsed = engine::parser::oracle::parse_oracle_text(
        MASTER_OF_CRUELTIES,
        "Master of Cruelties",
        &["First strike".into(), "Deathtouch".into()],
        &["Creature".into()],
        &["Demon".into()],
    );
    let trigger = parsed
        .triggers
        .iter()
        .find(|t| {
            matches!(
                t.mode,
                engine::types::triggers::TriggerMode::AttackerUnblocked
            )
        })
        .expect("unblocked trigger");
    let execute = trigger.execute.as_ref().expect("trigger effect");
    match &*execute.effect {
        engine::types::ability::Effect::SetLifeTotal { target, .. } => {
            assert_eq!(*target, TargetFilter::DefendingPlayer);
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

#[test]
fn declared_attack_sets_attacked_player_life_to_one() {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let master = add_master(&mut scenario);
    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(master, AttackTarget::Player(P2))])
        .expect("declare attackers");
    run_combat(&mut runner, master, P2);

    assert_eq!(life(&runner, P2), 1, "attacked player's life becomes 1");
    assert_eq!(life(&runner, P0), 20, "controller's life is untouched");
    assert_eq!(life(&runner, P1), 20, "non-attacked opponent is untouched");
}

#[test]
fn put_onto_battlefield_attacking_still_triggers_for_that_opponent() {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let kaalia = scenario
        .add_creature_from_oracle(P0, "Kaalia of the Vast", 2, 2, KAALIA)
        .with_keyword(Keyword::Flying)
        .id();
    let master = scenario
        .add_creature_to_hand_from_oracle(P0, "Master of Cruelties", 1, 4, MASTER_OF_CRUELTIES)
        .with_keyword(Keyword::FirstStrike)
        .with_keyword(Keyword::Deathtouch)
        .with_subtypes(vec!["Demon"])
        .id();
    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(kaalia, AttackTarget::Player(P2))])
        .expect("declare attackers");
    run_combat(&mut runner, master, P2);

    // CR 509.3c: Master entered attacking P2 and was unblocked. Its life-total
    // effect resolves before combat damage, then Kaalia deals 2.
    assert_eq!(life(&runner, P0), 20, "controller's life is untouched");
    assert_eq!(life(&runner, P1), 20, "non-attacked opponent is untouched");
    assert!(
        life(&runner, P2) <= 1,
        "attacked player's life became 1 before Kaalia's damage, got {}",
        life(&runner, P2)
    );
}

#[test]
fn unblocked_attack_on_planeswalker_does_not_trigger() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let master = add_master(&mut scenario);
    let walker = scenario
        .add_planeswalker_from_oracle(P1, "Test Walker", "Gideon", 5, "")
        .id();
    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(master, AttackTarget::Planeswalker(walker))])
        .expect("declare attackers");
    run_combat(&mut runner, master, P1);

    assert_eq!(life(&runner, P0), 20);
    assert_eq!(life(&runner, P1), 20);
}
