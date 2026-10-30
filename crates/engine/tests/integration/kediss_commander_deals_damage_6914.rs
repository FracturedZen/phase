//! Kediss, Emberclaw Familiar (#6914) — "Whenever a commander you control
//! deals combat damage to an opponent, it deals that much damage to each other
//! opponent."
//!
//! CR 603.7c + CR 120.1: "it" is the triggering commander, so the commander —
//! not Kediss — is the source of the damage dealt to each other opponent.
//! Source identity is observable through lifelink (CR 702.15b): a lifelinking
//! commander must gain its controller life for the redirected damage too.

use engine::game::combat::AttackTarget;
use engine::types::ability::{DamageSource, Effect, PlayerFilter};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

use super::rules::{GameScenario, P0, P1};

const P2: PlayerId = PlayerId(2);
const KEDISS_ORACLE: &str = "Whenever a commander you control deals combat damage to an opponent, it deals that much damage to each other opponent.";

#[test]
fn kediss_trigger_binds_triggering_commander_as_damage_source() {
    let parsed = engine::parser::oracle::parse_oracle_text(
        KEDISS_ORACLE,
        "Kediss, Emberclaw Familiar",
        &[],
        &["Creature".into()],
        &["Elemental".into(), "Lizard".into()],
    );
    let trigger = parsed.triggers.first().expect("Kediss has a trigger");
    let execute = trigger.execute.as_ref().expect("trigger has an effect");
    match &*execute.effect {
        Effect::DamageEachPlayer {
            player_filter,
            damage_source,
            ..
        } => {
            assert_eq!(*player_filter, PlayerFilter::OpponentOtherThanTriggering);
            assert_eq!(*damage_source, Some(DamageSource::TriggeringSource));
        }
        other => panic!("expected DamageEachPlayer, got {other:?}"),
    }
}

#[test]
fn kediss_redirected_damage_is_dealt_by_the_commander() {
    let mut scenario = GameScenario::new_n_player(3, 6914);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, "Kediss, Emberclaw Familiar", 2, 2, KEDISS_ORACLE);
    let commander = {
        let mut b = scenario.add_creature(P0, "Lifelink Commander", 3, 3);
        b.lifelink();
        b.id()
    };
    let mut runner = scenario.build();
    runner
        .state_mut()
        .objects
        .get_mut(&commander)
        .expect("commander exists")
        .is_commander = true;

    let p0_start = runner.life(P0);
    let p1_start = runner.life(P1);
    let p2_start = runner.life(P2);

    // 3-player game: pass priority around the table until attackers.
    for _ in 0..12 {
        if runner.waiting_for_kind() == "DeclareAttackers" {
            break;
        }
        let _ = runner.act(GameAction::PassPriority);
    }
    runner
        .act(GameAction::DeclareAttackers {
            attacks: vec![(commander, AttackTarget::Player(P1))],
            bands: vec![],
        })
        .expect("DeclareAttackers should succeed");
    // Drive blockers (none), combat damage, and the Kediss trigger to resolution.
    for _ in 0..60 {
        let phase = runner.state().phase;
        if matches!(phase, Phase::EndCombat | Phase::PostCombatMain)
            && runner.state().stack.is_empty()
        {
            break;
        }
        match &runner.state().waiting_for {
            WaitingFor::DeclareBlockers { .. } => {
                runner
                    .act(GameAction::DeclareBlockers {
                        assignments: vec![],
                    })
                    .expect("DeclareBlockers should succeed");
            }
            _ => {
                let _ = runner.act(GameAction::PassPriority);
            }
        }
    }

    assert_eq!(runner.life(P1), p1_start - 3, "combat damage to P1");
    assert_eq!(
        runner.life(P2),
        p2_start - 3,
        "Kediss trigger deals that much damage to each other opponent"
    );
    // CR 702.15b: the commander (lifelink) is the source of both damage events.
    assert_eq!(
        runner.life(P0),
        p0_start + 6,
        "lifelink must apply to the trigger's damage because the commander deals it"
    );
}
