//! Issue #10315 — The One Ring: a player with protection from everything can
//! still be attacked.
//!
//! CR 702.16j: protection from everything means the player can't be targeted,
//! enchanted, or blocked (blocking doesn't apply to players), and all damage
//! that would be dealt to them is prevented. Being attacked is none of these:
//! choosing whom a creature attacks (CR 508.1b) is not targeting (CR 115.1).
//! The One Ring ruling (2023-06-16): "Creatures can still attack you while you
//! have protection from everything, although combat damage that they would
//! deal to you will be prevented."
//!
//! https://github.com/phase-rs/phase/issues/10315

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::game::static_abilities::player_has_protection_from_everything;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

use super::rules::AttackTarget;
use crate::support::shared_card_db;

/// P0 casts the real The One Ring (so its "if you cast it" ETB grants P0
/// protection from everything), then the game moves to P1's precombat main
/// phase with a 3/3 ready to attack.
fn ring_protected_opponent(db: &engine::database::CardDatabase) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new_n_player(2, 7);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_life(P0, 20);
    let ring = scenario.add_real_card(P0, "The One Ring", Zone::Hand, db);
    scenario.with_mana_pool(
        P0,
        (0..4)
            .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
            .collect(),
    );
    let attacker = scenario.add_creature(P1, "Hill Giant", 3, 3).id();
    let mut runner = scenario.build();

    runner.cast(ring).resolve();
    runner.advance_until_stack_empty();
    assert_eq!(runner.state().objects[&ring].zone, Zone::Battlefield);
    assert!(
        player_has_protection_from_everything(runner.state(), P0),
        "reach-guard: casting The One Ring must give P0 protection from everything"
    );

    // Hand the turn to P1 (P0's protection lasts until P0's next turn).
    let state = runner.state_mut();
    state.turn_number += 1;
    state.active_player = P1;
    state.priority_player = P1;
    state.phase = Phase::PreCombatMain;
    state.waiting_for = WaitingFor::Priority { player: P1 };
    (runner, attacker)
}

#[test]
fn player_with_protection_from_everything_is_a_legal_attack_target() {
    let Some(db) = shared_card_db() else {
        return;
    };
    let (mut runner, attacker) = ring_protected_opponent(db);

    runner.advance_to_combat();
    let WaitingFor::DeclareAttackers {
        valid_attacker_ids,
        valid_attack_targets,
        ..
    } = runner.state().waiting_for.clone()
    else {
        panic!(
            "P1 must reach the declare-attackers choice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert!(valid_attacker_ids.contains(&attacker));
    assert!(
        valid_attack_targets.contains(&AttackTarget::Player(P0)),
        "CR 702.16j: protection from everything does not stop P0 being attacked; \
         got {valid_attack_targets:?}"
    );

    runner
        .declare_attackers(&[(attacker, AttackTarget::Player(P0))])
        .expect("declaring an attack on the protected player must be legal");
    assert!(runner
        .state()
        .combat
        .as_ref()
        .is_some_and(|c| c.attackers.iter().any(|a| a.object_id == attacker)));
}

/// Pass the post-declaration priority window, declare no blocks, and deal
/// combat damage.
fn finish_unblocked_combat(runner: &mut GameRunner) {
    if matches!(runner.state().waiting_for, WaitingFor::Priority { .. }) {
        runner.pass_both_players();
    }
    if matches!(
        runner.state().waiting_for,
        WaitingFor::DeclareBlockers { .. }
    ) {
        runner
            .declare_blockers(&[])
            .expect("P0 declares no blockers");
    }
    runner.combat_damage();
}

#[test]
fn combat_damage_to_the_protected_player_is_prevented() {
    let Some(db) = shared_card_db() else {
        return;
    };
    let (mut runner, attacker) = ring_protected_opponent(db);

    runner.advance_to_combat();
    runner
        .declare_attackers(&[(attacker, AttackTarget::Player(P0))])
        .expect("declaring an attack on the protected player must be legal");
    finish_unblocked_combat(&mut runner);

    assert_eq!(
        runner.life(P0),
        20,
        "CR 702.16j: the attack is legal but its combat damage to P0 is prevented"
    );
}

/// Control: the same unblocked attack without protection (The One Ring put
/// onto the battlefield without being cast, so its ETB doesn't apply) deals
/// its 3 damage, proving the prevented case above really reached damage.
#[test]
fn unprotected_player_takes_the_same_combat_damage() {
    let Some(db) = shared_card_db() else {
        return;
    };
    let mut scenario = GameScenario::new_n_player(2, 7);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_life(P0, 20);
    scenario.add_real_card(P0, "The One Ring", Zone::Battlefield, db);
    let attacker = scenario.add_creature(P1, "Hill Giant", 3, 3).id();
    let mut runner = scenario.build();
    assert!(!player_has_protection_from_everything(runner.state(), P0));
    let state = runner.state_mut();
    state.turn_number += 1;
    state.active_player = P1;
    state.priority_player = P1;
    state.waiting_for = WaitingFor::Priority { player: P1 };

    runner.advance_to_combat();
    runner
        .declare_attackers(&[(attacker, AttackTarget::Player(P0))])
        .expect("attacking an unprotected player is legal");
    finish_unblocked_combat(&mut runner);

    assert_eq!(runner.life(P0), 17);
}
