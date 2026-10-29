//! Phelia, Exuberant Shepherd (#10314): "Whenever Phelia attacks, exile up to
//! one other target nonland permanent. At the beginning of the next end step,
//! return that card to the battlefield under its owner's control. If it
//! entered under your control, put a +1/+1 counter on Phelia."
//!
//! CR 603.7a: the return and the conditional counter are both the delayed
//! triggered ability's effect, so the counter is placed when the card comes
//! back at the next end step — never when it is exiled. CR 608.2c: "If it
//! entered under your control" reads the result of the return instruction,
//! so a card returned under its owner's (an opponent's) control gives none.

use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const PHELIA: &str = "Flash\nWhenever Phelia attacks, exile up to one other target nonland permanent. At the beginning of the next end step, return that card to the battlefield under its owner's control. If it entered under your control, put a +1/+1 counter on Phelia.";

struct Board {
    runner: GameRunner,
    phelia: ObjectId,
    target: ObjectId,
}

fn board(target_owner: PlayerId) -> Board {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let phelia = scenario
        .add_creature_from_oracle(P0, "Phelia, Exuberant Shepherd", 2, 2, PHELIA)
        .id();
    let target = scenario
        .add_creature(target_owner, "Grizzly Bears", 2, 2)
        .id();
    Board {
        runner: scenario.build(),
        phelia,
        target,
    }
}

fn plus_one_counters(runner: &GameRunner, id: ObjectId) -> u32 {
    runner.state().objects[&id]
        .counters
        .get(&CounterType::Plus1Plus1)
        .copied()
        .unwrap_or(0) as u32
}

/// Step the game one action, answering combat and target prompts.
fn step(b: &mut Board) {
    let action = match &b.runner.state().waiting_for {
        WaitingFor::DeclareAttackers { .. } => GameAction::DeclareAttackers {
            attacks: vec![(b.phelia, AttackTarget::Player(P1))],
            bands: vec![],
        },
        WaitingFor::DeclareBlockers { .. } => GameAction::DeclareBlockers {
            assignments: vec![],
        },
        WaitingFor::TargetSelection { .. } | WaitingFor::TriggerTargetSelection { .. } => {
            GameAction::SelectTargets {
                targets: vec![TargetRef::Object(b.target)],
            }
        }
        _ => GameAction::PassPriority,
    };
    b.runner.act(action).expect("the game advances");
}

fn drive_until(b: &mut Board, done: impl Fn(&Board) -> bool) {
    for _ in 0..200 {
        if done(b) {
            return;
        }
        step(b);
    }
    panic!(
        "stop condition never reached; waiting_for = {:?}",
        b.runner.state().waiting_for
    );
}

fn in_zone(b: &Board, card: CardId, zone: Zone) -> bool {
    b.runner
        .state()
        .objects
        .values()
        .any(|obj| obj.card_id == card && obj.zone == zone)
}

fn run(target_owner: PlayerId) -> (u32, u32) {
    let mut b = board(target_owner);
    let card = b.runner.state().objects[&b.target].card_id;
    // Resolve the attack trigger: the target is exiled.
    drive_until(&mut b, |b| {
        in_zone(b, card, Zone::Exile) && b.runner.state().stack.is_empty()
    });
    let at_exile = plus_one_counters(&b.runner, b.phelia);
    // The delayed trigger returns it at the beginning of the end step.
    drive_until(&mut b, |b| {
        b.runner.state().phase == Phase::End
            && b.runner.state().stack.is_empty()
            && in_zone(b, card, Zone::Battlefield)
    });
    let after_return = plus_one_counters(&b.runner, b.phelia);
    (at_exile, after_return)
}

/// CR 603.7a: Phelia's own permanent comes back under its owner's (Phelia's
/// controller's) control, so the counter arrives with the return — not at exile.
#[test]
fn own_permanent_counter_waits_for_the_end_step_return() {
    let (at_exile, after_return) = run(P0);
    assert_eq!(at_exile, 0, "no counter while the card is still in exile");
    assert_eq!(
        after_return, 1,
        "the returned card entered under your control"
    );
}

/// CR 608.2c: an opponent's permanent returns under its owner's control, so
/// "if it entered under your control" is false and no counter is ever placed.
#[test]
fn opponents_permanent_never_gives_a_counter() {
    let (at_exile, after_return) = run(P1);
    assert_eq!(at_exile, 0, "no counter while the card is still in exile");
    assert_eq!(after_return, 0, "it entered under its owner's control");
}
