//! Issue #8114 — Braids, Arisen Nightmare: in a multiplayer game, the decline
//! consequence ("that player loses 2 life and you draw a card") must apply to
//! each opponent who declines, including when an earlier opponent accepts
//! and resolves an interactive sacrifice choice (CR 608.2c, CR 101.4).

use engine::game::scenario::GameScenario;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const BRAIDS_ORACLE: &str = "At the beginning of your end step, you may sacrifice an artifact, \
creature, enchantment, land, or planeswalker. If you do, each opponent may sacrifice a permanent \
of their choice that shares a card type with it. For each opponent who doesn't, that player loses \
2 life and you draw a card.";

fn life(state: &engine::types::game_state::GameState, player: PlayerId) -> i32 {
    state
        .players
        .iter()
        .find(|p| p.id == player)
        .expect("player")
        .life
}

fn hand_len(state: &engine::types::game_state::GameState, player: PlayerId) -> usize {
    state
        .players
        .iter()
        .find(|p| p.id == player)
        .expect("player")
        .hand
        .len()
}

/// Scenario 1: Opponent 1 has no matching permanent and declines, Opponents 2 and 3
/// have matching creatures and decline. All 3 opponents must lose 2 life and Braids'
/// controller must draw 3 cards (reproducing the Discord report for #8114).
#[test]
fn braids_opponent_without_matching_permanent_and_opponents_with_creatures_all_decline() {
    let mut scenario = GameScenario::new_n_player(4, 42);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let p2 = PlayerId(2);
    let p3 = PlayerId(3);

    scenario.at_phase(Phase::PostCombatMain);
    scenario
        .add_creature(p0, "Braids, Arisen Nightmare", 3, 3)
        .from_oracle_text(BRAIDS_ORACLE);
    let p0_creature = scenario.add_creature(p0, "Grizzly Bears", 2, 2).id();

    // P1 controls only a land (no creature)
    scenario.add_basic_land(p1, engine::types::mana::ManaColor::Green);
    // P2 controls a creature
    scenario.add_creature(p2, "Bear Cub", 2, 2);
    // P3 controls a creature
    scenario.add_creature(p3, "Scryb Sprites", 1, 1);

    scenario.add_card_to_library_top(p0, "Library Card 1");
    scenario.add_card_to_library_top(p0, "Library Card 2");
    scenario.add_card_to_library_top(p0, "Library Card 3");

    let mut runner = scenario.build();
    let p0_life_before = life(runner.state(), p0);
    let p1_life_before = life(runner.state(), p1);
    let p2_life_before = life(runner.state(), p2);
    let p3_life_before = life(runner.state(), p3);
    let p0_hand_before = hand_len(runner.state(), p0);

    runner.advance_to_end_step();

    for _ in 0..100 {
        if matches!(runner.state().waiting_for, WaitingFor::Priority { .. })
            && runner.state().stack.is_empty()
        {
            break;
        }
        match runner.state().waiting_for.clone() {
            WaitingFor::OrderTriggers { .. } => {
                runner
                    .act(GameAction::OrderTriggers { order: vec![0] })
                    .expect("order end-step triggers");
            }
            WaitingFor::OptionalEffectChoice { player, .. } => {
                let accept = player == p0;
                runner
                    .act(GameAction::DecideOptionalEffect { accept })
                    .expect("optional effect decision");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p0 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p0_creature],
                    })
                    .expect("controller sacrifice");
            }
            WaitingFor::Priority { .. } if !runner.state().stack.is_empty() => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("pass priority while stack resolving");
            }
            _ => {
                runner.act(GameAction::PassPriority).unwrap();
            }
        }
    }

    assert_eq!(life(runner.state(), p0), p0_life_before, "P0 loses no life");
    assert_eq!(
        life(runner.state(), p1),
        p1_life_before - 2,
        "P1 loses 2 life"
    );
    assert_eq!(
        life(runner.state(), p2),
        p2_life_before - 2,
        "P2 loses 2 life"
    );
    assert_eq!(
        life(runner.state(), p3),
        p3_life_before - 2,
        "P3 loses 2 life"
    );
    assert_eq!(
        hand_len(runner.state(), p0),
        p0_hand_before + 3,
        "P0 draws 3 cards (1 per declining opponent)"
    );
}

/// Scenario 2: P1 declines, P2 controls 2 creatures and accepts (choosing via
/// EffectZoneChoice), and P3 declines. P3 must still be prompted and execute their
/// decline branch, rather than being dropped when P2's decline condition evaluates false.
#[test]
fn braids_p2_accepts_via_effect_zone_choice_and_p3_declines() {
    let mut scenario = GameScenario::new_n_player(4, 42);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let p2 = PlayerId(2);
    let p3 = PlayerId(3);

    scenario.at_phase(Phase::PostCombatMain);
    scenario
        .add_creature(p0, "Braids, Arisen Nightmare", 3, 3)
        .from_oracle_text(BRAIDS_ORACLE);
    let p0_creature = scenario.add_creature(p0, "Grizzly Bears", 2, 2).id();

    // P1 controls a creature and declines
    scenario.add_creature(p1, "Elvish Mystic", 1, 1);
    // P2 controls 2 creatures and accepts
    let p2_creature = scenario.add_creature(p2, "Bear Cub", 2, 2).id();
    scenario.add_creature(p2, "Grizzly Bears", 2, 2);
    // P3 controls a creature and declines
    scenario.add_creature(p3, "Scryb Sprites", 1, 1);

    scenario.add_card_to_library_top(p0, "Library Card 1");
    scenario.add_card_to_library_top(p0, "Library Card 2");
    scenario.add_card_to_library_top(p0, "Library Card 3");

    let mut runner = scenario.build();
    let p0_life_before = life(runner.state(), p0);
    let p1_life_before = life(runner.state(), p1);
    let p2_life_before = life(runner.state(), p2);
    let p3_life_before = life(runner.state(), p3);
    let p0_hand_before = hand_len(runner.state(), p0);

    runner.advance_to_end_step();

    for _ in 0..100 {
        if matches!(runner.state().waiting_for, WaitingFor::Priority { .. })
            && runner.state().stack.is_empty()
        {
            break;
        }
        match runner.state().waiting_for.clone() {
            WaitingFor::OrderTriggers { .. } => {
                runner
                    .act(GameAction::OrderTriggers { order: vec![0] })
                    .expect("order end-step triggers");
            }
            WaitingFor::OptionalEffectChoice { player, .. } => {
                let accept = player == p0 || player == p2;
                runner
                    .act(GameAction::DecideOptionalEffect { accept })
                    .expect("optional effect decision");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p0 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p0_creature],
                    })
                    .expect("controller sacrifice");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p2 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p2_creature],
                    })
                    .expect("P2 sacrifice");
            }
            WaitingFor::Priority { .. } if !runner.state().stack.is_empty() => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("pass priority while stack resolving");
            }
            _ => {
                runner.act(GameAction::PassPriority).unwrap();
            }
        }
    }

    assert_eq!(life(runner.state(), p0), p0_life_before, "P0 loses no life");
    assert_eq!(
        life(runner.state(), p1),
        p1_life_before - 2,
        "P1 loses 2 life"
    );
    assert_eq!(
        life(runner.state(), p2),
        p2_life_before,
        "P2 sacrificed, loses NO life"
    );
    assert_eq!(
        life(runner.state(), p3),
        p3_life_before - 2,
        "P3 declined, loses 2 life"
    );
    assert_eq!(
        hand_len(runner.state(), p0),
        p0_hand_before + 2,
        "P0 draws 2 cards (1 for P1, 1 for P3)"
    );
}

/// Scenario 3: P1 and P2 both control 2 creatures and accept via EffectZoneChoice,
/// and P3 declines. P3 must still execute their decline branch.
#[test]
fn braids_p1_and_p2_accept_via_effect_zone_choice_and_p3_declines() {
    let mut scenario = GameScenario::new_n_player(4, 42);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let p2 = PlayerId(2);
    let p3 = PlayerId(3);

    scenario.at_phase(Phase::PostCombatMain);
    scenario
        .add_creature(p0, "Braids, Arisen Nightmare", 3, 3)
        .from_oracle_text(BRAIDS_ORACLE);
    let p0_creature = scenario.add_creature(p0, "Grizzly Bears", 2, 2).id();

    // P1 controls 2 creatures and accepts
    let p1_creature = scenario.add_creature(p1, "Elvish Mystic", 1, 1).id();
    scenario.add_creature(p1, "Llanowar Elves", 1, 1);
    // P2 controls 2 creatures and accepts
    let p2_creature = scenario.add_creature(p2, "Bear Cub", 2, 2).id();
    scenario.add_creature(p2, "Grizzly Bears", 2, 2);
    // P3 controls 2 creatures and declines
    scenario.add_creature(p3, "Scryb Sprites", 1, 1);
    scenario.add_creature(p3, "Birds of Paradise", 0, 1);

    scenario.add_card_to_library_top(p0, "Library Card 1");
    scenario.add_card_to_library_top(p0, "Library Card 2");
    scenario.add_card_to_library_top(p0, "Library Card 3");

    let mut runner = scenario.build();
    let p0_life_before = life(runner.state(), p0);
    let p1_life_before = life(runner.state(), p1);
    let p2_life_before = life(runner.state(), p2);
    let p3_life_before = life(runner.state(), p3);
    let p0_hand_before = hand_len(runner.state(), p0);

    runner.advance_to_end_step();

    for _ in 0..100 {
        if matches!(runner.state().waiting_for, WaitingFor::Priority { .. })
            && runner.state().stack.is_empty()
        {
            break;
        }
        match runner.state().waiting_for.clone() {
            WaitingFor::OrderTriggers { .. } => {
                runner
                    .act(GameAction::OrderTriggers { order: vec![0] })
                    .expect("order end-step triggers");
            }
            WaitingFor::OptionalEffectChoice { player, .. } => {
                let accept = player == p0 || player == p1 || player == p2;
                runner
                    .act(GameAction::DecideOptionalEffect { accept })
                    .expect("optional effect decision");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p0 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p0_creature],
                    })
                    .expect("controller sacrifice");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p1 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p1_creature],
                    })
                    .expect("P1 sacrifice");
            }
            WaitingFor::EffectZoneChoice { player, .. } if player == p2 => {
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![p2_creature],
                    })
                    .expect("P2 sacrifice");
            }
            WaitingFor::Priority { .. } if !runner.state().stack.is_empty() => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("pass priority while stack resolving");
            }
            _ => {
                runner.act(GameAction::PassPriority).unwrap();
            }
        }
    }

    assert_eq!(life(runner.state(), p0), p0_life_before, "P0 loses no life");
    assert_eq!(
        life(runner.state(), p1),
        p1_life_before,
        "P1 sacrificed, loses NO life"
    );
    assert_eq!(
        life(runner.state(), p2),
        p2_life_before,
        "P2 sacrificed, loses NO life"
    );
    assert_eq!(
        life(runner.state(), p3),
        p3_life_before - 2,
        "P3 declined, loses 2 life"
    );
    assert_eq!(
        hand_len(runner.state(), p0),
        p0_hand_before + 1,
        "P0 draws 1 card for P3"
    );
}
