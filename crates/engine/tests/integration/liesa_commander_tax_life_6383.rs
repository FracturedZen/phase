//! Issue #6383 — Liesa, Shroud of Dusk: "Rather than pay {2} for each previous
//! time you've cast this spell from the command zone this game, pay 2 life
//! that many times."
//!
//! CR 903.8: commander tax is {2} per previous cast from the command zone.
//! CR 118.9 + CR 119.4: Liesa pays that tax as 2 life per previous cast
//! instead; the ruling (2020-11-10) says only the commander tax is replaced,
//! and paying mana for it instead is not an option.

use engine::ai_support::legal_actions_full;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::format::FormatConfig;
use engine::types::game_state::CastPaymentMode;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::statics::StaticMode;
use engine::types::zones::Zone;

const LIESA: &str = "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 2 life that many times.\nFlying, lifelink\nWhenever a player casts a spell, they lose 2 life.";

fn colorless(count: usize) -> Vec<ManaUnit> {
    (0..count)
        .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
        .collect()
}

/// A commander in P0's command zone with mana cost {2}, `previous_casts`
/// recorded command-zone casts, P0 at `life` with `mana` colorless in pool.
fn setup(
    name: &str,
    oracle: &str,
    previous_casts: u32,
    life: i32,
    mana: usize,
) -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new_with_format(FormatConfig::commander(), 2, 7);
    scenario.at_phase(Phase::PreCombatMain);
    let id = scenario
        .add_creature_to_hand_from_oracle(P0, name, 5, 5, oracle)
        .with_mana_cost(ManaCost::generic(2))
        .id();
    scenario.with_commander(id);
    scenario.with_life(P0, life);
    scenario.with_mana_pool(P0, colorless(mana));
    let mut runner = scenario.build();
    if previous_casts > 0 {
        runner
            .state_mut()
            .commander_cast_count
            .insert(id, previous_casts);
    }
    (runner, id)
}

fn offers_cast(runner: &GameRunner, card: ObjectId) -> bool {
    let (actions, _, _) = legal_actions_full(runner.state());
    actions.iter().any(
        |action| matches!(action, GameAction::CastSpell { object_id, .. } if *object_id == card),
    )
}

fn cast(runner: &mut GameRunner, card: ObjectId) {
    let card_id = runner.state().objects[&card].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: card,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("casting the commander is legal");
}

#[test]
fn liesa_parses_as_commander_tax_paid_with_life_static() {
    let (runner, liesa) = setup("Liesa, Shroud of Dusk", LIESA, 0, 20, 2);
    let obj = &runner.state().objects[&liesa];
    assert!(obj
        .static_definitions
        .as_slice()
        .iter()
        .any(|def| def.mode == StaticMode::CommanderTaxPaidWithLife));
}

#[test]
fn liesa_recast_pays_life_not_mana_for_commander_tax() {
    // Two previous casts: tax would be {4}; Liesa pays 4 life instead, so {2}
    // in pool is enough.
    let (mut runner, liesa) = setup("Liesa, Shroud of Dusk", LIESA, 2, 20, 2);
    assert!(offers_cast(&runner, liesa));
    cast(&mut runner, liesa);
    assert_eq!(runner.state().objects[&liesa].zone, Zone::Stack);
    assert_eq!(runner.life(P0), 16);
    assert_eq!(runner.state().commander_cast_count.get(&liesa), Some(&3));
}

#[test]
fn liesa_first_cast_pays_no_life() {
    let (mut runner, liesa) = setup("Liesa, Shroud of Dusk", LIESA, 0, 20, 2);
    cast(&mut runner, liesa);
    assert_eq!(runner.state().objects[&liesa].zone, Zone::Stack);
    assert_eq!(runner.life(P0), 20);
}

#[test]
fn liesa_cannot_be_cast_without_enough_life_for_the_tax() {
    // CR 119.4: tax of 4 life with only 3 life — and mana can't substitute.
    let (runner, liesa) = setup("Liesa, Shroud of Dusk", LIESA, 2, 3, 10);
    assert!(!offers_cast(&runner, liesa));
}

#[test]
fn ordinary_commander_still_pays_mana_tax() {
    let (runner, cmd) = setup("Plain Commander", "Flying", 1, 20, 2);
    assert!(!offers_cast(&runner, cmd));
    let (mut runner, cmd) = setup("Plain Commander", "Flying", 1, 20, 4);
    assert!(offers_cast(&runner, cmd));
    cast(&mut runner, cmd);
    assert_eq!(runner.life(P0), 20);
}
