//! Regression for issue #8064: Bloodthirster's "can't attack a player it has
//! already attacked this turn" lowered to a blanket "can't attack", so it could
//! never attack at all.
//!
//! CR 508.1b + CR 508.1c: the restriction scopes only the attack target — a
//! player this creature was declared attacking earlier this turn. Its first
//! attack is legal; after its damage trigger grants an additional combat phase
//! (CR 500.8), it can attack a different player but not the same one again.
//!
//! https://github.com/phase-rs/phase/issues/8064

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

use super::rules::AttackTarget;

const P2: PlayerId = PlayerId(2);

const BLOODTHIRSTER_ORACLE: &str = "Whenever this creature deals combat damage to a player, \
untap it. After this phase, there is an additional combat phase.\n\
This creature can't attack a player it has already attacked this turn.";

fn setup() -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let bt = scenario
        .add_creature_from_oracle(P0, "Bloodthirster", 6, 6, BLOODTHIRSTER_ORACLE)
        .id();
    let runner = scenario.build();
    (runner, bt)
}

fn targets_for(runner: &GameRunner, attacker: ObjectId) -> Vec<AttackTarget> {
    match &runner.state().waiting_for {
        WaitingFor::DeclareAttackers {
            valid_attacker_ids,
            valid_attack_targets_by_attacker,
            ..
        } => {
            assert!(
                valid_attacker_ids.contains(&attacker),
                "Bloodthirster must be a valid attacker"
            );
            valid_attack_targets_by_attacker
                .as_ref()
                .and_then(|m| m.get(&attacker).cloned())
                .unwrap_or_default()
        }
        other => panic!("expected DeclareAttackers, got {other:?}"),
    }
}

/// Drive the first combat (no blocks) until the additional combat's
/// declare-attackers step.
fn drive_to_second_declaration(runner: &mut GameRunner) {
    for _ in 0..200 {
        match runner.state().waiting_for.clone() {
            WaitingFor::DeclareAttackers { .. } => return,
            WaitingFor::DeclareBlockers { .. } => {
                runner
                    .act(GameAction::DeclareBlockers {
                        assignments: vec![],
                    })
                    .expect("no blocks");
            }
            WaitingFor::Priority { .. } => {
                runner.act(GameAction::PassPriority).expect("pass");
            }
            other => panic!("unexpected prompt before second combat: {other:?}"),
        }
    }
    panic!(
        "never reached the additional combat's declaration; phase = {:?}",
        runner.state().phase
    );
}

/// Pass priority around the table until the active player is asked to declare
/// attackers (three players each pass in main and at beginning of combat).
fn to_declare_attackers(runner: &mut GameRunner) {
    for _ in 0..20 {
        if matches!(
            runner.state().waiting_for,
            WaitingFor::DeclareAttackers { .. }
        ) {
            return;
        }
        runner.act(GameAction::PassPriority).expect("pass");
    }
    panic!(
        "never reached DeclareAttackers: {:?}",
        runner.state().waiting_for
    );
}

#[test]
fn bloodthirster_can_attack_first_then_not_the_same_player_again() {
    let (mut runner, bt) = setup();

    to_declare_attackers(&mut runner);
    runner
        .act(GameAction::DeclareAttackers {
            attacks: vec![(bt, AttackTarget::Player(P1))],
            bands: vec![],
        })
        .expect("#8064: Bloodthirster's first attack this turn must be legal");

    let p1_life = runner.state().players[1].life;
    drive_to_second_declaration(&mut runner);
    assert_eq!(
        runner.state().players[1].life,
        p1_life - 6,
        "first combat damage must land on P1"
    );
    assert!(
        !runner.state().objects[&bt].tapped,
        "the damage trigger must untap Bloodthirster"
    );

    let targets = targets_for(&runner, bt);
    assert!(
        !targets.contains(&AttackTarget::Player(P1)),
        "P1 was already attacked this turn and must not be offered: {targets:?}"
    );
    assert!(
        targets.contains(&AttackTarget::Player(P2)),
        "P2 has not been attacked this turn and must be offered: {targets:?}"
    );

    let mut probe = runner.state().clone();
    let err = engine::game::engine::apply_as_current(
        &mut probe,
        GameAction::DeclareAttackers {
            attacks: vec![(bt, AttackTarget::Player(P1))],
            bands: vec![],
        },
    );
    assert!(
        err.is_err(),
        "attacking P1 again in the additional combat must be rejected"
    );

    runner
        .act(GameAction::DeclareAttackers {
            attacks: vec![(bt, AttackTarget::Player(P2))],
            bands: vec![],
        })
        .expect("attacking a different player in the additional combat is legal");
}

#[test]
fn bloodthirster_first_combat_offers_every_opponent() {
    let (mut runner, bt) = setup();
    to_declare_attackers(&mut runner);
    let targets = targets_for(&runner, bt);
    assert!(targets.contains(&AttackTarget::Player(P1)), "{targets:?}");
    assert!(targets.contains(&AttackTarget::Player(P2)), "{targets:?}");
}
