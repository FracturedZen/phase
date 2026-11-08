//! Issue #7460: "Target opponent chooses N [of those cards]" after a search or
//! a reveal names the opponent announced as the spell's target (CR 115.1 +
//! CR 601.2c + CR 608.2c), not "an opponent" picked at resolution. In a
//! multiplayer game the targeted opponent — and only that opponent — makes
//! the choice.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

/// Verbatim Scryfall Oracle text.
const INTUITION: &str = "Search your library for any three cards and reveal them. Target \
opponent chooses one. Put that card into your hand and the rest into your graveyard. Then \
shuffle.";

/// Verbatim Scryfall Oracle text.
const GIFTS_UNGIVEN: &str = "Search your library for up to four cards with different names \
and reveal them. Target opponent chooses two of those cards. Put the chosen cards into your \
graveyard and the rest into your hand. Then shuffle.";

/// Thran Tome's activated-ability effect text (verbatim), cast here as a
/// spell so the test isolates the chooser binding.
const THRAN_TOME: &str = "Reveal the top three cards of your library. Target opponent \
chooses one of those cards. Put that card into your graveyard, then draw two cards.";

const P2: PlayerId = PlayerId(2);

fn cast_targeting(name: &str, oracle: &str, target: PlayerId) -> GameRunner {
    let mut scenario = GameScenario::new_n_player(4, 7);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Card A", "Card B", "Card C", "Card D", "Card E"]);
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, name, false, oracle)
        .with_mana_cost(ManaCost::zero())
        .id();
    let mut runner = scenario.build();
    runner.cast(spell).target_player(target).resolve();
    runner
}

/// Selects the cards the search prompt asks for. (The search count itself is
/// not under test here; this module isolates who makes the later choice.)
fn finish_search(runner: &mut GameRunner) {
    let WaitingFor::SearchChoice { cards, count, .. } = runner.state().waiting_for.clone() else {
        panic!(
            "expected SearchChoice, got {:?}",
            runner.state().waiting_for
        );
    };
    runner
        .act(GameAction::SelectCards {
            cards: cards.into_iter().take(count).collect(),
        })
        .expect("search selection resumes the spell");
}

fn assert_chooser(runner: &GameRunner, expected: PlayerId, count: usize) {
    let WaitingFor::ChooseFromZoneChoice {
        player, count: n, ..
    } = runner.state().waiting_for.clone()
    else {
        panic!(
            "expected the targeted opponent's ChooseFromZoneChoice, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(
        player, expected,
        "the targeted opponent must make the choice"
    );
    assert_eq!(n, count);
}

#[test]
fn intuition_targeted_non_default_opponent_chooses() {
    let mut runner = cast_targeting("Intuition", INTUITION, P2);
    finish_search(&mut runner);
    assert_chooser(&runner, P2, 1);
}

#[test]
fn gifts_ungiven_targeted_non_default_opponent_chooses() {
    let mut runner = cast_targeting("Gifts Ungiven", GIFTS_UNGIVEN, P2);
    finish_search(&mut runner);
    assert_chooser(&runner, P2, 2);
}

#[test]
fn thran_tome_reveal_targeted_non_default_opponent_chooses() {
    let mut runner = cast_targeting("Thran Tome Effect", THRAN_TOME, P2);
    assert_chooser(&runner, P2, 1);
    let WaitingFor::ChooseFromZoneChoice { cards, .. } = runner.state().waiting_for.clone() else {
        unreachable!()
    };
    let chosen = cards[1];
    runner
        .act(GameAction::SelectCards {
            cards: vec![chosen],
        })
        .expect("targeted opponent selects a revealed card");
    assert!(runner.state().players[0].graveyard.contains(&chosen));
    assert_eq!(runner.state().players[0].hand.len(), 2);
    assert!(runner.state().stack.is_empty());
}

#[test]
fn intuition_two_player_target_still_chooses() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_library_top(P0, &["Card A", "Card B", "Card C"]);
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Intuition", false, INTUITION)
        .with_mana_cost(ManaCost::zero())
        .id();
    let mut runner = scenario.build();
    runner.cast(spell).target_player(P1).resolve();
    finish_search(&mut runner);
    assert_chooser(&runner, P1, 1);
}
