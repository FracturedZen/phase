//! #10295: a "source of your choice" may be an object referred to by an object
//! on the stack even after it has left its zone (CR 609.7a). Jade Monolith can
//! choose a Mogg Fanatic that was sacrificed to activate its pending ability,
//! and the Fanatic's damage is then redirected.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const JADE: &str = "{1}: The next time a source of your choice would deal damage to target creature this turn, that source deals that damage to you instead.";
const FANATIC: &str = "Sacrifice this creature: It deals 1 damage to any target.";

struct Board {
    runner: GameRunner,
    jade: ObjectId,
    fanatic: ObjectId,
    bear: ObjectId,
    unrelated: ObjectId,
}

fn board() -> Board {
    let mut s = GameScenario::new();
    s.at_phase(Phase::PreCombatMain);
    let jade = s.add_artifact_from_oracle(P0, "Jade Monolith", JADE).id();
    let fanatic = s
        .add_creature_from_oracle(P0, "Mogg Fanatic", 1, 1, FANATIC)
        .id();
    let bear = s.add_creature(P1, "Bear", 2, 2).id();
    // An unrelated card in a graveyard that nothing pending refers to.
    let unrelated = s.add_creature_to_graveyard(P0, "Unrelated Card", 3, 3).id();
    let mut runner = s.build();
    runner.state_mut().add_mana_to_pool(
        P0,
        ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
    );
    Board {
        runner,
        jade,
        fanatic,
        bear,
        unrelated,
    }
}

fn activate_targeting(runner: &mut GameRunner, source: ObjectId, target: ObjectId) {
    runner
        .act(GameAction::ActivateAbility {
            source_id: source,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("activate the printed ability");
    // A single legal target is auto-selected (Jade's only creature target once
    // the Fanatic has been sacrificed); otherwise answer the target prompt.
    if matches!(
        runner.state().waiting_for,
        WaitingFor::TargetSelection { .. }
    ) {
        runner
            .act(GameAction::ChooseTarget {
                target: Some(TargetRef::Object(target)),
            })
            .expect("choose the target");
    }
    let top = runner.state().stack.last().expect("ability on the stack");
    assert_eq!(top.source_id, source);
    assert_eq!(
        top.ability().map(|a| a.targets.clone()),
        Some(vec![TargetRef::Object(target)])
    );
}

fn pass_until(runner: &mut GameRunner, done: impl Fn(&GameRunner) -> bool) {
    for _ in 0..16 {
        if done(runner) {
            return;
        }
        runner.act(GameAction::PassPriority).expect("pass priority");
    }
    panic!("condition not reached: {:?}", runner.state().waiting_for);
}

fn life(runner: &GameRunner, player: PlayerId) -> i32 {
    runner.life(player)
}

/// Fanatic is sacrificed; its ability is on the stack. Jade resolves above it
/// and reaches its source choice.
fn reach_source_choice(b: &mut Board) -> Vec<ObjectId> {
    activate_targeting(&mut b.runner, b.fanatic, b.bear);
    assert_eq!(
        b.runner.state().objects[&b.fanatic].zone,
        Zone::Graveyard,
        "Fanatic was sacrificed as the cost"
    );
    activate_targeting(&mut b.runner, b.jade, b.bear);
    assert_eq!(b.runner.state().stack.len(), 2);
    pass_until(&mut b.runner, |r| {
        matches!(r.state().waiting_for, WaitingFor::DamageSourceChoice { .. })
    });
    let WaitingFor::DamageSourceChoice {
        player, options, ..
    } = &b.runner.state().waiting_for
    else {
        unreachable!()
    };
    assert_eq!(*player, P0);
    options.clone()
}

#[test]
fn sacrificed_fanatic_referred_by_its_stacked_ability_is_a_source_option() {
    let mut b = board();
    let options = reach_source_choice(&mut b);
    assert!(
        options.contains(&b.fanatic),
        "CR 609.7a: the Fanatic its pending ability refers to is choosable from the graveyard"
    );
    assert!(
        !options.contains(&b.unrelated),
        "an unrelated graveyard card is not referred to by anything pending"
    );
}

#[test]
fn chosen_departed_fanatic_damage_is_redirected() {
    let mut b = board();
    reach_source_choice(&mut b);
    let p0_before = life(&b.runner, P0);
    b.runner
        .act(GameAction::ChooseDamageSource { source: b.fanatic })
        .expect("choose the departed Fanatic");
    pass_until(&mut b.runner, |r| r.state().stack.is_empty());

    assert_eq!(
        b.runner.state().objects[&b.bear].damage_marked,
        0,
        "the Fanatic's damage to the Bear is redirected"
    );
    assert_eq!(
        life(&b.runner, P0),
        p0_before - 1,
        "dealt to Jade's controller instead"
    );
}
