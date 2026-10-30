//! Integration tests for GitHub issue #8075 — Indoraptor, the Perfect Hybrid.
//!
//! "Enrage — Whenever Indoraptor is dealt damage, choose an opponent at random.
//! Indoraptor deals damage equal to its power to that player unless they
//! sacrifice a nontoken creature of their choice."
//!
//! Before the fix, "that player" after "choose an opponent at random" parsed as
//! the event-context `TriggeringPlayer` (for an enrage trigger, Indoraptor's own
//! controller), and the anaphoric "unless they" payer read `ability.targets`,
//! found no declared player, and silently skipped the payment.
//!
//! CR 608.2c: instructions are followed in order, so "that player" refers to the
//! opponent the preceding `Choose` selected (CR 109.4). CR 118.12a: the player
//! named by "unless they …" — that same chosen opponent — decides whether to pay.

use engine::game::scenario::{GameRunner, GameScenario};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{ControllerRef, Effect, TargetFilter, TypedFilter};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const INDORAPTOR_ORACLE: &str = "Enrage — Whenever Indoraptor is dealt damage, choose an \
     opponent at random. Indoraptor deals damage equal to its power to that player unless \
     they sacrifice a nontoken creature of their choice.";

/// The same effect body on an end-step trigger, so the runtime test can fire it
/// through the real turn structure without a separate damage source.
const END_STEP_ORACLE: &str = "At the beginning of your end step, choose an opponent at \
     random. Raptor deals damage equal to its power to that player unless they sacrifice a \
     nontoken creature of their choice.";

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

fn chosen_player_zero() -> TargetFilter {
    TargetFilter::Typed(TypedFilter {
        controller: Some(ControllerRef::ChosenPlayer { index: 0 }),
        ..Default::default()
    })
}

/// CR 608.2c + CR 109.4 + CR 118.12a: both the damage recipient and the
/// unless-payer bind to the chosen opponent, not the triggering player.
#[test]
fn indoraptor_that_player_and_payer_bind_to_chosen_opponent() {
    let parsed = parse_oracle_text(
        INDORAPTOR_ORACLE,
        "Indoraptor, the Perfect Hybrid",
        &[],
        &["Creature".to_string()],
        &["Dinosaur".to_string(), "Mutant".to_string()],
    );
    let execute = parsed.triggers[0]
        .execute
        .as_ref()
        .expect("enrage trigger must have an execute body");
    assert!(
        matches!(execute.effect.as_ref(), Effect::Choose { .. }),
        "first instruction is the random opponent choice: {:?}",
        execute.effect
    );
    let damage = execute
        .sub_ability
        .as_ref()
        .expect("damage instruction follows the choice");
    match damage.effect.as_ref() {
        Effect::DealDamage { target, .. } => assert_eq!(
            target,
            &chosen_player_zero(),
            "\"that player\" must be the chosen opponent"
        ),
        other => panic!("expected DealDamage, got {other:?}"),
    }
    let unless = damage
        .unless_pay
        .as_ref()
        .expect("unless-sacrifice modifier must be attached");
    assert_eq!(
        unless.payer,
        chosen_player_zero(),
        "\"unless they\" must be paid by the chosen opponent"
    );
}

/// CR 608.2c + CR 109.4: the spell form of the class ("Choose an opponent at
/// random. ~ deals 3 damage to that player.") binds the same way.
#[test]
fn choose_opponent_at_random_spell_that_player_is_chosen() {
    let parsed = parse_oracle_text(
        "Choose an opponent at random. This spell deals 3 damage to that player.",
        "Test Random Bolt",
        &[],
        &["Sorcery".to_string()],
        &[],
    );
    let spell = &parsed.abilities[0];
    let damage = spell.sub_ability.as_ref().expect("damage follows the choice");
    match damage.effect.as_ref() {
        Effect::DealDamage { target, .. } => assert_eq!(target, &chosen_player_zero()),
        other => panic!("expected DealDamage, got {other:?}"),
    }
}

fn life(runner: &GameRunner, player: PlayerId) -> i32 {
    runner.state().players[player.0 as usize].life
}

fn on_battlefield(runner: &GameRunner, id: ObjectId) -> bool {
    runner
        .state()
        .objects
        .get(&id)
        .is_some_and(|o| o.zone == Zone::Battlefield)
}

/// Advance from main phase 1 to the unless-payment prompt of the end-step
/// trigger, returning the prompted player.
fn advance_to_unless_prompt(runner: &mut GameRunner) -> PlayerId {
    for _ in 0..200 {
        match &runner.state().waiting_for {
            WaitingFor::UnlessPayment { player, .. } => return *player,
            WaitingFor::Priority { .. } => {
                runner.act(GameAction::PassPriority).expect("pass priority");
            }
            WaitingFor::DeclareAttackers { .. } => {
                runner
                    .act(GameAction::DeclareAttackers {
                        attacks: vec![],
                        bands: vec![],
                    })
                    .expect("declare no attackers");
            }
            other => panic!("unexpected wait state before unless prompt: {other:?}"),
        }
    }
    panic!("never reached the unless-payment prompt");
}

struct Setup {
    runner: GameRunner,
    p1_creature: ObjectId,
    p2_creature: ObjectId,
}

fn setup() -> Setup {
    let mut scenario = GameScenario::new_n_player(3, 8075);
    scenario.at_phase(Phase::PreCombatMain);
    for &pid in &[P0, P1, P2] {
        scenario.with_library_top(pid, &["Lib A", "Lib B", "Lib C", "Lib D"]);
    }
    scenario.add_creature_from_oracle(P0, "Raptor", 4, 4, END_STEP_ORACLE);
    let p1_creature = scenario.add_creature(P1, "P1 Beast", 1, 1).id();
    let p2_creature = scenario.add_creature(P2, "P2 Beast", 1, 1).id();
    Setup {
        runner: scenario.build(),
        p1_creature,
        p2_creature,
    }
}

/// CR 118.12a: the randomly chosen opponent — never the controller — is asked
/// to pay; declining deals the damage to that opponent only.
#[test]
fn chosen_opponent_declines_and_takes_damage() {
    let Setup { mut runner, .. } = setup();
    let payer = advance_to_unless_prompt(&mut runner);
    assert!(
        payer == P1 || payer == P2,
        "the unless prompt must go to an opponent, got {payer:?}"
    );
    let other = if payer == P1 { P2 } else { P1 };

    runner
        .act(GameAction::PayUnlessCost { pay: false })
        .expect("decline the sacrifice");
    runner.advance_until_stack_empty();

    assert_eq!(life(&runner, payer), 16, "chosen opponent takes 4 damage");
    assert_eq!(life(&runner, P0), 20, "the controller is never the recipient");
    assert_eq!(life(&runner, other), 20, "the unchosen opponent is untouched");
}

/// CR 118.12a: paying the unless cost (sacrificing a nontoken creature the
/// chosen opponent controls) prevents the damage.
#[test]
fn chosen_opponent_sacrifices_and_prevents_damage() {
    let Setup {
        mut runner,
        p1_creature,
        p2_creature,
    } = setup();
    let payer = advance_to_unless_prompt(&mut runner);
    let (payer_creature, other_creature) = if payer == P1 {
        (p1_creature, p2_creature)
    } else {
        (p2_creature, p1_creature)
    };

    runner
        .act(GameAction::PayUnlessCost { pay: true })
        .expect("pay the sacrifice");
    // CR 701.21a: the chosen opponent picks which of their permanents to
    // sacrifice — only their own nontoken creature is eligible.
    match &runner.state().waiting_for {
        WaitingFor::WardSacrificeChoice {
            player, permanents, ..
        } => {
            assert_eq!(*player, payer, "the chosen opponent makes the sacrifice");
            assert_eq!(
                permanents,
                &vec![payer_creature],
                "only the chosen opponent's own nontoken creature is eligible"
            );
        }
        other => panic!("expected the sacrifice choice, got {other:?}"),
    }
    runner
        .act(GameAction::SelectCards {
            cards: vec![payer_creature],
        })
        .expect("sacrifice the creature");
    runner.advance_until_stack_empty();

    assert!(
        !on_battlefield(&runner, payer_creature),
        "the chosen opponent sacrificed their own creature"
    );
    assert!(on_battlefield(&runner, other_creature));
    assert_eq!(life(&runner, payer), 20, "paying prevents the damage");
    assert_eq!(life(&runner, P0), 20);
}
