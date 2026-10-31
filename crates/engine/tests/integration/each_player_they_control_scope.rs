//! Runtime + AST regression: "each player … [count] of [X] THEY control" must
//! bind the per-player count to the ITERATING player, not the caster.
//!
//! Three cards exhibit the bug:
//!   - Acidic Soil — "deals damage to each player equal to the number of lands
//!     they control" (parser-only; `DamageEachPlayer` already resolves per player).
//!   - Biorhythm + Shaman of Forgotten Ways — "Each player's life total becomes
//!     the number of creatures they control" (parser + `SetLifeTotal` resolver).
//!
//! Before the fix the count carried `controller: You`, so every player took /
//! was set to the CASTER's count. After the fix the count carries
//! `ControllerRef::ScopedPlayer` and the resolver rebinds `scoped_player` per
//! recipient.
//!
//! CR 109.5: "they/their control" in an each-player effect refers to the player
//! the effect is being applied to.
//! CR 119.5: a "becomes [N]" effect sets each affected player's life total.
//! CR 120.3: damage to each player is dealt to each player in turn.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::parser::oracle_effect::parse_effect_chain;
use engine::types::ability::{
    AbilityKind, ControllerRef, Effect, QuantityExpr, QuantityRef, TargetFilter,
};
use engine::types::actions::GameAction;
use engine::types::format::FormatConfig;
use engine::types::game_state::CastPaymentMode;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;

const ACIDIC_SOIL: &str =
    "Acidic Soil deals damage to each player equal to the number of lands they control.";
const BIORHYTHM: &str = "Each player's life total becomes the number of creatures they control.";
const SHAMAN_ACTIVATED: &str = "Each player's life total becomes the number of creatures they \
     control. Activate only if creatures you control have total power 8 or greater.";

// --- Regression baselines (must stay UNSCOPED) --------------------------------
const WORLDFIRE_LIFE: &str = "Each player's life total becomes 1.";
const REPAY_IN_KIND: &str =
    "Each player's life total becomes the lowest life total among all players.";
const OKETRA: &str = "Your life total becomes equal to your starting life total.";

/// Extract the `controller` of the lone `ObjectCount` quantity in `amount`.
fn object_count_controller(amount: &QuantityExpr) -> Option<ControllerRef> {
    match amount {
        QuantityExpr::Ref {
            qty:
                QuantityRef::ObjectCount {
                    filter: TargetFilter::Typed(tf),
                },
        } => tf.controller.clone(),
        _ => None,
    }
}

// =============================== AST tests ====================================

/// CR 109.5: Acidic Soil's per-player land count binds to `ScopedPlayer`.
#[test]
fn acidic_soil_count_is_scoped_player() {
    let def = parse_effect_chain(ACIDIC_SOIL, AbilityKind::Spell);
    match &*def.effect {
        Effect::DamageEachPlayer { amount, .. } => {
            assert_eq!(
                object_count_controller(amount),
                Some(ControllerRef::ScopedPlayer),
                "land count must scope to the iterating player, not the caster (You)"
            );
        }
        other => panic!("expected DamageEachPlayer, got {other:?}"),
    }
}

/// CR 109.5 + CR 119.5: Biorhythm's per-player creature count binds to `ScopedPlayer`.
#[test]
fn biorhythm_count_is_scoped_player() {
    let def = parse_effect_chain(BIORHYTHM, AbilityKind::Spell);
    match &*def.effect {
        Effect::SetLifeTotal { target, amount } => {
            assert_eq!(
                *target,
                TargetFilter::AllPlayers,
                "each player's life total"
            );
            assert_eq!(
                object_count_controller(amount),
                Some(ControllerRef::ScopedPlayer),
                "creature count must scope to the iterating player, not the caster (You)"
            );
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

/// Shaman of Forgotten Ways: the SetLifeTotal amount scopes per player even when
/// the activation-condition clause ("Activate only if creatures you control have
/// total power 8 or greater") trails the effect body — the "you control"
/// condition does NOT bleed into the count's scope. The activation condition
/// itself is parsed by the ability-level parser (not `parse_effect_chain`) and
/// resolved by `resolve_quantity_for_ability_condition`, a separate caster-scoped
/// path untouched by this fix.
#[test]
fn shaman_set_life_amount_is_scoped_player() {
    let def = parse_effect_chain(SHAMAN_ACTIVATED, AbilityKind::Activated);
    match &*def.effect {
        Effect::SetLifeTotal { target, amount } => {
            assert_eq!(*target, TargetFilter::AllPlayers);
            assert_eq!(
                object_count_controller(amount),
                Some(ControllerRef::ScopedPlayer),
                "creature count must scope per player, not to the caster"
            );
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

// --- No-regression AST baselines ---------------------------------------------

/// Worldfire: numeric "becomes 1" stays a `Fixed` literal (no scope to apply).
#[test]
fn worldfire_stays_fixed_one() {
    let def = parse_effect_chain(WORLDFIRE_LIFE, AbilityKind::Spell);
    match &*def.effect {
        Effect::SetLifeTotal { target, amount } => {
            assert_eq!(*target, TargetFilter::AllPlayers);
            assert!(
                matches!(amount, QuantityExpr::Fixed { value: 1 }),
                "Worldfire must stay Fixed{{1}}, got {amount:?}"
            );
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

/// Repay in Kind: cross-player extremum stays `LifeTotal{AllPlayers{Min}}` — it
/// carries no "they control" count, so the per-player scope must not corrupt it.
#[test]
fn repay_in_kind_stays_cross_player_extremum() {
    let def = parse_effect_chain(REPAY_IN_KIND, AbilityKind::Spell);
    match &*def.effect {
        Effect::SetLifeTotal { target, amount } => {
            assert_eq!(*target, TargetFilter::AllPlayers);
            // No ObjectCount → controller scope is irrelevant; must NOT be an
            // ObjectCount at all (it's a cross-player life extremum).
            assert!(
                object_count_controller(amount).is_none(),
                "Repay in Kind must stay a cross-player extremum, not an ObjectCount, got {amount:?}"
            );
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

/// Oketra's Last Mercy: "your life total becomes …" stays Controller/You, not
/// AllPlayers and not ScopedPlayer.
#[test]
fn oketra_stays_controller_scoped() {
    let def = parse_effect_chain(OKETRA, AbilityKind::Spell);
    match &*def.effect {
        Effect::SetLifeTotal { target, .. } => {
            assert_ne!(
                *target,
                TargetFilter::AllPlayers,
                "your life total is single-player (Controller), not AllPlayers"
            );
        }
        other => panic!("expected SetLifeTotal, got {other:?}"),
    }
}

// ============================= Runtime tests ==================================

/// Acidic Soil runtime: P0 controls 5 lands, P1 controls 2. Each player takes
/// damage equal to the number of lands THEY control. Reverting the parser fix
/// (count → `You`) makes BOTH players take the caster's count (5) and fails.
///
/// CR 120.3: each player is dealt the per-player amount.
#[test]
fn acidic_soil_damages_each_player_by_their_own_land_count() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);

    // P0 (caster) controls 5 lands, P1 controls 2 (A != B).
    for _ in 0..5 {
        scenario.add_basic_land(P0, engine::types::mana::ManaColor::Red);
    }
    for _ in 0..2 {
        scenario.add_basic_land(P1, engine::types::mana::ManaColor::Green);
    }

    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Acidic Soil", false, ACIDIC_SOIL)
        .with_mana_cost(ManaCost::zero())
        .id();

    let mut runner = scenario.build();
    let outcome = runner.cast(spell).resolve();

    // P0 takes 5 (its own lands), P1 takes 2 (its own lands).
    outcome.assert_life_delta(P0, -5);
    outcome.assert_life_delta(P1, -2);
}

/// Biorhythm runtime: P0 controls 3 creatures, P1 controls 1. Each player's life
/// total becomes the number of creatures THEY control. Reverting the parser fix
/// (count → `You`) OR the resolver fix (amount resolved once, pre-loop, from the
/// caster's perspective) sets BOTH players to the caster's count (3) and fails.
///
/// The `AllPlayers`-scoped `SetLifeTotal` is not a targeted spell (CR 115.1),
/// so it casts with no targets through the production pipeline (`runner.cast(spell).resolve()`)
/// over a battlefield staged by `GameScenario`. The PARSED Biorhythm effect is used
/// verbatim, so the per-player `ScopedPlayer` count is exercised end-to-end.
///
/// CR 119.5: life total is set to the per-player creature count.
#[test]
fn biorhythm_sets_each_player_life_to_their_own_creature_count() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);

    // P0 (caster) controls 3 creatures, P1 controls 1 (C != D).
    for _ in 0..3 {
        scenario.add_vanilla(P0, 1, 1);
    }
    scenario.add_vanilla(P1, 1, 1);

    let biorhythm = scenario
        .add_spell_to_hand_from_oracle(P0, "Biorhythm", false, BIORHYTHM)
        .with_mana_cost(ManaCost::generic(0))
        .id();

    let mut runner = scenario.build();
    runner.cast(biorhythm).resolve();

    // P0 life -> 3 (its own creatures), P1 life -> 1 (its own creature).
    assert_eq!(
        runner.life(P0),
        3,
        "P0 life set to its own creature count (3)"
    );
    assert_eq!(
        runner.life(P1),
        1,
        "P1 life set to its own creature count (1)"
    );
}

/// CR 904.13b + CR 810.9d + CR 119.5, runtime: Repay in Kind's parsed
/// cross-player extremum resolves in Archenemy Commander with the archenemy
/// in seat 1. The Min over team totals is the archenemy's 10, not the
/// heroes' shared 60, so every side is set to 10. Reverting the seat-1
/// dedup-key fix collapses the archenemy into the heroes' key and reads 60:
/// the archenemy would rise to 60 and fail below.
#[test]
fn repay_in_kind_seat_one_archenemy_sets_all_sides_to_ten() {
    let mut config = FormatConfig::archenemy_commander();
    config.archenemy_player = Some(PlayerId(1));
    let mut scenario = GameScenario::new_with_format(config, 4, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let repay = scenario
        .add_spell_to_hand_from_oracle(P0, "Repay in Kind", false, REPAY_IN_KIND)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let mut runner = scenario.build();

    // Hand turn and priority to P0 so P0 can cast Repay in Kind.
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = engine::types::game_state::WaitingFor::Priority { player: P0 };

    // Archenemy at 10; heroes splitting a shared 60.
    runner.state_mut().players[1].life = 10;
    runner.state_mut().players[0].life = 20;
    runner.state_mut().players[2].life = 20;
    runner.state_mut().players[3].life = 20;

    runner.cast(repay).resolve();

    let state = runner.state();
    assert_eq!(
        state.players[1].life, 10,
        "seat-1 archenemy stays at the minimum 10 (a collapsed key reads 60)"
    );
    let heroes_shared: i32 = [0, 2, 3].iter().map(|&i| state.players[i].life).sum();
    assert_eq!(
        heroes_shared, 10,
        "the heroes' shared total is set to the minimum 10"
    );
}

/// Issue #10455: Untargeted AllPlayers SetLifeTotal spells (Repay in Kind,
/// Biorhythm, Worldfire) must not demand target slots during announcement (CR 115.1).
/// `GameAction::CastSpell` with `targets: vec![]` must succeed.
#[test]
fn repay_in_kind_cast_spell_action_with_empty_targets_succeeds() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let repay = scenario
        .add_spell_to_hand_from_oracle(P0, "Repay in Kind", false, REPAY_IN_KIND)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let mut runner = scenario.build();
    let card_id = runner.state().objects[&repay].card_id;

    runner.state_mut().players[0].life = 20;
    runner.state_mut().players[1].life = 7;

    let result = runner.act(GameAction::CastSpell {
        object_id: repay,
        card_id,
        targets: vec![],
        payment_mode: CastPaymentMode::Auto,
    });
    assert!(
        result.is_ok(),
        "casting Repay in Kind with targets: vec![] must succeed (CR 115.1), got {result:?}"
    );
    runner.advance_until_stack_empty();

    assert_eq!(
        runner.life(P0),
        7,
        "P0 life total becomes lowest life total (7)"
    );
    assert_eq!(
        runner.life(P1),
        7,
        "P1 life total becomes lowest life total (7)"
    );
}

/// Issue #10455: Worldfire's untargeted AllPlayers SetLifeTotal effect must cast
/// without targeting and set every player's life total to 1 (CR 115.1, CR 119.5).
#[test]
fn worldfire_cast_sets_all_players_life_to_one() {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let worldfire_oracle = "Exile all permanents. Exile all cards from all hands and graveyards. Each player's life total becomes 1.";
    let worldfire = scenario
        .add_spell_to_hand_from_oracle(P0, "Worldfire", false, worldfire_oracle)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    scenario.add_vanilla(P0, 2, 2);
    scenario.add_vanilla(P1, 3, 3);

    let mut runner = scenario.build();
    runner.cast(worldfire).resolve();

    assert_eq!(runner.life(P0), 1, "P0 life becomes 1 (CR 119.5)");
    assert_eq!(runner.life(P1), 1, "P1 life becomes 1 (CR 119.5)");
}
