//! CR 303.4 + CR 608.2d + CR 102.2 (issue #9611): Lynde's upkeep chooses a Curse
//! attached to its controller and moves that Curse onto an opponent.

use crate::game::effects::attach::attach_to_player;
use crate::game::game_object::AttachTarget;
use crate::game::scenario::{GameRunner, GameScenario, P0, P1};
use crate::types::actions::GameAction;
use crate::types::game_state::WaitingFor;
use crate::types::identifiers::ObjectId;
use crate::types::phase::Phase;
use crate::types::player::PlayerId;

const LYNDE: &str = "Deathtouch\nWhenever a Curse is put into your graveyard from the battlefield, return it to the battlefield attached to you at the beginning of the next end step.\nAt the beginning of your upkeep, you may attach a Curse attached to you to one of your opponents. If you do, draw two cards.";

fn curse(scenario: &mut GameScenario, name: &str) -> ObjectId {
    scenario
        .add_enchantment_from_oracle(P0, name, "Enchant player")
        .with_subtypes(vec!["Aura", "Curse"])
        .id()
}

fn hand_len(runner: &GameRunner, player: PlayerId) -> usize {
    runner
        .state()
        .players
        .iter()
        .find(|entry| entry.id == player)
        .map(|entry| entry.hand.len())
        .unwrap_or(0)
}

fn attached_to(runner: &GameRunner, id: ObjectId) -> Option<AttachTarget> {
    runner.state().objects.get(&id).and_then(|obj| obj.attached_to)
}

fn pass_until_prompt(runner: &mut GameRunner) {
    for _ in 0..16 {
        if !matches!(runner.state().waiting_for, WaitingFor::Priority { .. }) {
            return;
        }
        runner
            .act(GameAction::PassPriority)
            .expect("priority pass while the upkeep trigger is on the stack");
    }
    panic!(
        "upkeep trigger never left priority: {:?}",
        runner.state().waiting_for
    );
}

fn accept_upkeep(runner: &mut GameRunner) {
    let WaitingFor::OptionalEffectChoice { player, .. } = runner.state().waiting_for.clone() else {
        panic!(
            "Lynde's upkeep must offer the may-attach choice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(player, P0);
    runner
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accepting the upkeep attach");
}

fn prepare(players: u8, curses: &[(&str, PlayerId)]) -> (GameRunner, Vec<ObjectId>) {
    let mut scenario = if players == 2 {
        GameScenario::new()
    } else {
        GameScenario::new_n_player(players, 42)
    };
    scenario.at_phase(Phase::Untap);
    scenario
        .add_creature_from_oracle(P0, "Lynde, Cheerful Tormentor", 2, 4, LYNDE)
        .id();
    let ids: Vec<ObjectId> = curses
        .iter()
        .map(|(name, _host)| curse(&mut scenario, name))
        .collect();
    scenario.add_card_to_library_top(P0, "Island");
    scenario.add_card_to_library_top(P0, "Island");
    let mut runner = scenario.build();
    for (id, (_name, host)) in ids.iter().zip(curses.iter()) {
        attach_to_player(runner.state_mut(), *id, *host);
        assert_eq!(
            attached_to(&runner, *id),
            Some(AttachTarget::Player(*host)),
            "setup must attach {id:?} to {host:?}"
        );
    }
    runner.advance_to_upkeep();
    pass_until_prompt(&mut runner);
    (runner, ids)
}

#[test]
fn lynde_upkeep_attaches_the_curse_on_you_to_the_only_opponent_and_draws() {
    let (mut runner, ids) = prepare(2, &[("Curse of You", P0), ("Curse of Them", P1)]);
    let on_you = ids[0];
    let on_them = ids[1];
    accept_upkeep(&mut runner);
    assert!(
        !matches!(
            runner.state().waiting_for,
            WaitingFor::AttachHostPlayerChoice { .. }
        ),
        "a duel has one legal opponent, so the host is not a prompt: {:?}",
        runner.state().waiting_for
    );
    assert_eq!(attached_to(&runner, on_you), Some(AttachTarget::Player(P1)));
    assert_eq!(attached_to(&runner, on_them), Some(AttachTarget::Player(P1)));
    assert_eq!(hand_len(&runner, P0), 2);
    assert!(
        matches!(runner.state().waiting_for, WaitingFor::Priority { .. }),
        "the draw must finish the ability: {:?}",
        runner.state().waiting_for
    );
}

#[test]
fn lynde_upkeep_decline_leaves_the_curses_and_draws_nothing() {
    let (mut runner, ids) = prepare(2, &[("Curse of You", P0), ("Curse of Them", P1)]);
    let WaitingFor::OptionalEffectChoice { player, .. } = runner.state().waiting_for.clone()
    else {
        panic!(
            "Lynde's upkeep must offer the may-attach choice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(player, P0);
    runner
        .act(GameAction::DecideOptionalEffect { accept: false })
        .expect("declining the upkeep attach");
    assert_eq!(attached_to(&runner, ids[0]), Some(AttachTarget::Player(P0)));
    assert_eq!(attached_to(&runner, ids[1]), Some(AttachTarget::Player(P1)));
    assert_eq!(hand_len(&runner, P0), 0);
}

#[test]
fn lynde_upkeep_asks_which_opponent_when_several_are_legal() {
    let p2 = PlayerId(2);
    let (mut runner, ids) = prepare(3, &[("Curse of You", P0)]);
    accept_upkeep(&mut runner);
    let WaitingFor::AttachHostPlayerChoice {
        player, candidates, ..
    } = runner.state().waiting_for.clone()
    else {
        panic!(
            "two opponents must be a player-host choice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(player, P0);
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&P1));
    assert!(candidates.contains(&p2));
    runner
        .act(GameAction::ChooseAttachHostPlayer { opponent: p2 })
        .expect("choosing the third player");
    assert_eq!(attached_to(&runner, ids[0]), Some(AttachTarget::Player(p2)));
    assert_eq!(hand_len(&runner, P0), 2);
    assert!(
        matches!(runner.state().waiting_for, WaitingFor::Priority { .. }),
        "the draw must follow the chosen host: {:?}",
        runner.state().waiting_for
    );
}

#[test]
fn lynde_upkeep_picks_one_curse_then_one_opponent() {
    let p2 = PlayerId(2);
    let (mut runner, ids) = prepare(3, &[("First Curse", P0), ("Second Curse", P0)]);
    let chosen = ids[0];
    let other = ids[1];
    accept_upkeep(&mut runner);
    let WaitingFor::EffectZoneChoice { player, cards, .. } = runner.state().waiting_for.clone()
    else {
        panic!(
            "two curses must be chosen before the opponent, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(player, P0);
    assert!(cards.contains(&chosen));
    assert!(cards.contains(&other));
    runner
        .act(GameAction::SelectCards {
            cards: vec![chosen],
        })
        .expect("choosing which curse moves");
    let WaitingFor::AttachHostPlayerChoice { candidates, .. } = runner.state().waiting_for.clone()
    else {
        panic!(
            "the chosen curse still needs an opponent, got {:?}",
            runner.state().waiting_for
        );
    };
    assert!(candidates.contains(&p2));
    runner
        .act(GameAction::ChooseAttachHostPlayer { opponent: p2 })
        .expect("choosing the third player");
    assert_eq!(attached_to(&runner, chosen), Some(AttachTarget::Player(p2)));
    assert_eq!(attached_to(&runner, other), Some(AttachTarget::Player(P0)));
    assert_eq!(hand_len(&runner, P0), 2);
}
