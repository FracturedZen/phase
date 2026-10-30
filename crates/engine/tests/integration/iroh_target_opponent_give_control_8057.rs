//! Issue #8057: Iroh, Tea Master — "you may have target opponent gain control
//! of target permanent you control" offered no player target in multiplayer.
//!
//! CR 115.1 + CR 601.2c + CR 603.3d: "target opponent" / "target player" as the
//! subject of "gains control of" is a target announced with the ability's other
//! targets. CR 613.1b: control of the chosen permanent then changes to THAT
//! player. Before the fix the recipient was an untargeted opponent filter, so a
//! 3+ player game had no player slot, the resolver could not pick among several
//! opponents, and the permanent never changed hands. The same parse drives
//! Donate ("Target player gains control …") and Harmless Offering ("Target
//! opponent gains control …").

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const IROH: &str = "When Iroh enters, create a Food token.\nAt the beginning of combat on your turn, you may have target opponent gain control of target permanent you control. When you do, create a 1/1 white Ally creature token. Put a +1/+1 counter on that token for each permanent you own that your opponents control.";

const P2: PlayerId = PlayerId(2);

/// Answers every target prompt with `recipient` for the player slot and `gift`
/// for the object slot, recording each slot's legal targets.
fn choose(
    runner: &mut engine::game::scenario::GameRunner,
    recipient: PlayerId,
    gift: ObjectId,
    slots_seen: &mut Vec<Vec<TargetRef>>,
) -> bool {
    let wf = runner.state().waiting_for.clone();
    let (WaitingFor::TriggerTargetSelection {
        target_slots,
        selection,
        ..
    }
    | WaitingFor::TargetSelection {
        target_slots,
        selection,
        ..
    }) = wf
    else {
        return false;
    };
    let legal = target_slots[selection.current_slot].legal_targets.clone();
    let pick = if legal.iter().any(|t| matches!(t, TargetRef::Player(_))) {
        TargetRef::Player(recipient)
    } else {
        TargetRef::Object(gift)
    };
    assert!(legal.contains(&pick), "{pick:?} must be legal in {legal:?}");
    slots_seen.push(legal);
    runner
        .act(GameAction::ChooseTarget { target: Some(pick) })
        .expect("target choice accepted");
    true
}

fn run_iroh(players: u8, recipient: PlayerId) -> (PlayerId, Vec<u32>, Vec<Vec<TargetRef>>) {
    let mut scenario = GameScenario::new_n_player(players, 42);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, "Iroh, Tea Master", 2, 2, IROH);
    let bear = scenario.add_creature(P0, "Gift Bear", 2, 2).id();
    let mut runner = scenario.build();
    let mut slots_seen = Vec::new();

    for _ in 0..80 {
        if choose(&mut runner, recipient, bear, &mut slots_seen) {
            continue;
        }
        let result = match runner.state().waiting_for.clone() {
            WaitingFor::OptionalEffectChoice { .. } => {
                runner.act(GameAction::DecideOptionalEffect { accept: true })
            }
            WaitingFor::OrderTriggers { triggers, .. } => runner.act(GameAction::OrderTriggers {
                order: (0..triggers.len()).collect(),
            }),
            WaitingFor::DeclareAttackers { .. } => break,
            _ => runner.act(GameAction::PassPriority),
        };
        result.expect("action accepted");
    }

    let state = runner.state();
    let ally_counters = state
        .objects
        .values()
        .filter(|o| o.is_token && o.name == "Ally")
        .map(|o| {
            o.counters
                .get(&CounterType::Plus1Plus1)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    (state.objects[&bear].controller, ally_counters, slots_seen)
}

/// CR 115.1 + CR 603.3d + CR 613.1b: in a 3-player game the trigger announces
/// an opponent target (both opponents legal, never the controller), the chosen
/// opponent gains control, and the reflexive Ally counts that permanent.
#[test]
fn iroh_three_player_gives_permanent_to_chosen_opponent() {
    let (controller, ally_counters, slots) = run_iroh(3, P2);

    let player_slot = slots
        .iter()
        .find(|legal| legal.iter().any(|t| matches!(t, TargetRef::Player(_))))
        .expect("the trigger must announce a target opponent");
    assert!(player_slot.contains(&TargetRef::Player(P1)));
    assert!(player_slot.contains(&TargetRef::Player(P2)));
    assert!(
        !player_slot.contains(&TargetRef::Player(P0)),
        "\"target opponent\" excludes the controller"
    );
    assert_eq!(controller, P2, "the chosen opponent gains control");
    assert_eq!(
        ally_counters,
        vec![1],
        "one Ally with a counter for the one owned permanent an opponent controls"
    );
}

/// Two-player control case: the sole opponent is still announced and receives
/// the permanent.
#[test]
fn iroh_two_player_gives_permanent_to_opponent() {
    let (controller, ally_counters, _) = run_iroh(2, P1);
    assert_eq!(controller, P1);
    assert_eq!(ally_counters, vec![1]);
}

fn cast_gift_spell(name: &str, oracle: &str, recipient: PlayerId) -> (PlayerId, Vec<TargetRef>) {
    let mut scenario = GameScenario::new_n_player(3, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let bear = scenario.add_creature(P0, "Gift Bear", 2, 2).id();
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, name, false, oracle)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![],
            generic: 0,
        })
        .id();
    let mut runner = scenario.build();
    let card_id = runner.state().objects[&spell].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: spell,
            card_id,
            targets: vec![],
            payment_mode: Default::default(),
        })
        .expect("cast accepted");

    let mut slots_seen = Vec::new();
    for _ in 0..20 {
        if choose(&mut runner, recipient, bear, &mut slots_seen) {
            continue;
        }
        if runner.state().stack.is_empty() {
            break;
        }
        runner.act(GameAction::PassPriority).expect("pass accepted");
    }
    let player_slot = slots_seen
        .into_iter()
        .find(|legal| legal.iter().any(|t| matches!(t, TargetRef::Player(_))))
        .expect("the spell must announce its recipient as a target");
    (runner.state().objects[&bear].controller, player_slot)
}

/// CR 115.1 + CR 613.1b: Donate — "target player" is any player, and the
/// chosen one gains control in a 3-player game.
#[test]
fn donate_three_player_gives_permanent_to_target_player() {
    let (controller, player_slot) = cast_gift_spell(
        "Donate",
        "Target player gains control of target permanent you control.",
        P2,
    );
    assert!(player_slot.contains(&TargetRef::Player(P0)));
    assert!(player_slot.contains(&TargetRef::Player(P1)));
    assert!(player_slot.contains(&TargetRef::Player(P2)));
    assert_eq!(controller, P2);
}

/// CR 115.1 + CR 102.3: Harmless Offering — "target opponent" offers only
/// opponents, and the chosen one gains control.
#[test]
fn harmless_offering_three_player_gives_permanent_to_target_opponent() {
    let (controller, player_slot) = cast_gift_spell(
        "Harmless Offering",
        "Target opponent gains control of target permanent you control.",
        P1,
    );
    assert!(!player_slot.contains(&TargetRef::Player(P0)));
    assert!(player_slot.contains(&TargetRef::Player(P2)));
    assert_eq!(controller, P1);
}
