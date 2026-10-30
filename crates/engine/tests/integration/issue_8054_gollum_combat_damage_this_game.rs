//! Issue #8054 — Gollum, Obsessed Stalker.
//!
//! Oracle: "Skulk / At the beginning of your end step, each opponent dealt
//! combat damage this game by a creature named Gollum, Obsessed Stalker loses
//! life equal to the amount of life you gained this turn."
//!
//! Before the fix the relative clause "dealt combat damage this game by a
//! creature named …" was silently dropped, so the trigger's player scope was a
//! bare `Opponent` and EVERY opponent lost life. The fix:
//!   - CR 120.1 + CR 608.2i: a never-cleared ledger of damage dealt to players
//!     (`GameState::damage_dealt_to_players_this_game`), carrying the same
//!     damage-time source snapshot as the per-turn ledger;
//!   - `PlayerFilter::OpponentDealtDamage { scope: DamageHistoryScope::ThisGame }`
//!     reading that ledger;
//!   - a general "each opponent [who was ]dealt <kind> damage this turn/this
//!     game [by <source>]" subject clause (CR 120.2a, CR 120.9), with the
//!     source parsed by the shared target-phrase parser (CR 201.2 "named").

use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    DamageHistoryScope, DamageKindFilter, FilterProp, PlayerFilter, TargetFilter,
};
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const P2: PlayerId = PlayerId(2);
const GOLLUM: &str = "Gollum, Obsessed Stalker";
const GOLLUM_TEXT: &str = "Skulk (This creature can't be blocked by creatures with greater power.)\nAt the beginning of your end step, each opponent dealt combat damage this game by a creature named Gollum, Obsessed Stalker loses life equal to the amount of life you gained this turn.";

fn filter_names_gollum(f: &TargetFilter) -> bool {
    match f {
        TargetFilter::Typed(tf) => tf
            .properties
            .iter()
            .any(|p| matches!(p, FilterProp::Named { name } if name == GOLLUM)),
        TargetFilter::Named { name } => name == GOLLUM,
        TargetFilter::And { filters } => filters.iter().any(filter_names_gollum),
        _ => false,
    }
}

/// CR 120.2a + CR 120.9 + CR 201.2: the end-step trigger's recipient scope is
/// the game-scoped, combat-only, Gollum-named damage look-back — not every
/// opponent.
#[test]
fn gollum_trigger_scope_is_game_scoped_named_combat_damage() {
    let parsed = parse_oracle_text(
        GOLLUM_TEXT,
        GOLLUM,
        &["Skulk".to_string()],
        &["Creature".to_string()],
        &["Halfling".to_string(), "Horror".to_string()],
    );
    let trigger = parsed
        .triggers
        .iter()
        .find(|t| t.phase == Some(Phase::End))
        .expect("end-step trigger");
    let exec = trigger.execute.as_ref().expect("trigger body");
    match exec.player_scope.as_ref() {
        Some(PlayerFilter::OpponentDealtDamage {
            kind: DamageKindFilter::CombatOnly,
            source: Some(source),
            min_sources: 1,
            scope: DamageHistoryScope::ThisGame,
        }) => assert!(
            filter_names_gollum(source),
            "source filter must be 'a creature named {GOLLUM}', got {source:?}"
        ),
        other => panic!("expected game-scoped OpponentDealtDamage scope, got {other:?}"),
    }
}

/// Three-player game: Gollum attacks P1, a vanilla bear attacks P2 (both
/// unblocked). Returns the runner after combat damage, with Gollum's id.
fn combat_setup() -> (GameRunner, ObjectId) {
    let mut s = GameScenario::new_n_player(3, 8054);
    s.at_phase(Phase::PreCombatMain);
    let gollum = s
        .add_creature(P0, GOLLUM, 2, 2)
        .from_oracle_text_with_keywords(&["Skulk"], GOLLUM_TEXT)
        .id();
    let bear = s.add_creature(P0, "Grizzly Bears", 2, 2).id();
    for p in [P0, P1, P2] {
        for _ in 0..5 {
            s.add_card_to_library_top(p, "Plains");
        }
    }
    let mut r = s.build();
    r.advance_to_combat();
    r.declare_attackers(&[
        (gollum, AttackTarget::Player(P1)),
        (bear, AttackTarget::Player(P2)),
    ])
    .expect("declare attackers");
    let _ = r.combat_damage();
    assert_eq!(r.life(P1), 18, "Gollum dealt 2 combat damage to P1");
    assert_eq!(r.life(P2), 18, "the bear dealt 2 combat damage to P2");
    (r, gollum)
}

fn gain_life_and_run_end_step(r: &mut GameRunner, gained: u32) {
    // CR 119.3: model "you gained N life this turn" directly; the trigger reads
    // `LifeGainedThisTurn { Controller }`.
    r.state_mut().players[0].life_gained_this_turn = gained;
    r.advance_to_end_step();
    r.advance_until_stack_empty();
}

/// Only the opponent dealt combat damage by Gollum loses life; the opponent
/// damaged by a differently-named creature does not.
#[test]
fn gollum_end_step_drains_only_opponent_gollum_damaged() {
    let (mut r, _gollum) = combat_setup();
    gain_life_and_run_end_step(&mut r, 3);
    assert_eq!(
        r.life(P1),
        15,
        "P1 was dealt combat damage by Gollum: loses 3 (life gained this turn)"
    );
    assert_eq!(
        r.life(P2),
        18,
        "P2 was dealt combat damage only by a creature not named Gollum: no loss"
    );
}

/// CR 120.1 "this game": the look-back survives the per-turn damage ledger
/// being cleared at the turn boundary (simulated here by clearing it, as the
/// cleanup step does), so damage Gollum dealt on an earlier turn still counts.
#[test]
fn gollum_combat_damage_look_back_spans_the_game() {
    let (mut r, _gollum) = combat_setup();
    r.state_mut().damage_dealt_this_turn.clear();
    gain_life_and_run_end_step(&mut r, 4);
    assert_eq!(
        r.life(P1),
        14,
        "earlier Gollum damage still counts this game"
    );
    assert_eq!(r.life(P2), 18, "P2 never took Gollum damage");
}
