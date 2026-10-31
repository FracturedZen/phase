//! Issue #7458 — Choking Vines + mass population tracked set consumers.
//!
//! CR 509.1h + CR 608.2c:
//! "X target attacking creatures become blocked. Choking Vines deals 1 damage to each of those creatures."
//!
//! The `BecomeBlocked` head must publish the blocked creatures for
//! `DamageAll { target: TrackedSet }` to deal 1 damage to each of them.

use engine::game::combat::{unblocked_attackers, AttackTarget};
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::Effect;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaCost, ManaCostShard, ManaPool, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const CHOKING_VINES: &str = "Cast this spell only during the declare blockers step.\n\
    X target attacking creatures become blocked. Choking Vines deals 1 damage to each of those creatures.";

const WINTER_BLAST: &str =
    "Tap X target creatures. Winter Blast deals 2 damage to each of those creatures with flying.";

fn advance_to_declare_blockers_and_give_priority(runner: &mut GameRunner, caster: PlayerId) {
    for _ in 0..20 {
        let wf = runner.state().waiting_for.clone();
        if runner.state().phase == Phase::DeclareBlockers {
            let state = runner.state_mut();
            state.priority_player = caster;
            state.waiting_for = WaitingFor::Priority { player: caster };
            return;
        }
        let acted = match wf {
            WaitingFor::Priority { .. } => runner.act(GameAction::PassPriority),
            WaitingFor::DeclareBlockers { .. } => runner.act(GameAction::DeclareBlockers {
                assignments: vec![],
            }),
            other => panic!("unexpected waiting state before declare-blockers: {other:?}"),
        };
        acted.expect("combat advance action");
    }
    panic!("did not reach the declare-blockers step");
}

#[test]
fn choking_vines_parses_become_blocked_then_damage_all() {
    let parsed = parse_oracle_text(
        CHOKING_VINES,
        "Choking Vines",
        &[],
        &["Instant".to_string()],
        &[],
    );
    assert_eq!(parsed.abilities.len(), 1, "one spell ability");
    let ability = &parsed.abilities[0];
    assert!(
        matches!(*ability.effect, Effect::BecomeBlocked { .. }),
        "head effect must be BecomeBlocked, got {:?}",
        ability.effect
    );
    let sub = ability
        .sub_ability
        .as_ref()
        .expect("damage sub-ability must compose after BecomeBlocked");
    assert!(
        matches!(*sub.effect, Effect::DamageAll { .. }),
        "sub effect must be DamageAll, got {:?}",
        sub.effect
    );
}

#[test]
fn choking_vines_deals_damage_to_blocked_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let attacker = scenario.add_creature(P0, "Attacking Elf", 1, 1).id();
    let vines = scenario
        .add_spell_to_hand_from_oracle(P1, "Choking Vines", true, CHOKING_VINES)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::X, ManaCostShard::Green],
            generic: 0,
        })
        .id();

    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(attacker, AttackTarget::Player(P1))])
        .expect("declare attackers");
    advance_to_declare_blockers_and_give_priority(&mut runner, P1);

    // Provide mana for {1}{G} (X=1) in the declare blockers step where P1 has priority.
    if let Some(p) = runner.state_mut().players.iter_mut().find(|p| p.id == P1) {
        p.mana_pool = ManaPool::from_units(vec![
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]),
        ]);
    }

    // P1 casts Choking Vines targeting the attacker with X=1.
    runner
        .cast(vines)
        .x(1)
        .target_objects(&[attacker])
        .resolve();

    // The attacker is blocked.
    assert!(
        !unblocked_attackers(runner.state()).contains(&attacker),
        "attacker must become blocked"
    );

    // CR 509.1h + CR 608.2c + CR 120.3:
    // Choking Vines must deal 1 damage to each of those creatures.
    // The 1/1 attacker receives 1 damage and is destroyed by SBAs.
    assert_eq!(
        runner.state().objects.get(&attacker).map(|o| o.zone),
        Some(Zone::Graveyard),
        "the 1/1 attacker dealt 1 lethal damage by Choking Vines must be in the graveyard"
    );
}

#[test]
fn choking_vines_multi_target_damages_each_blocked_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let elf = scenario.add_creature(P0, "Attacking Elf", 1, 1).id();
    let bear = scenario.add_creature(P0, "Attacking Bear", 2, 2).id();
    let giant = scenario.add_creature(P0, "Attacking Giant", 3, 3).id();
    let vines = scenario
        .add_spell_to_hand_from_oracle(P1, "Choking Vines", true, CHOKING_VINES)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::X, ManaCostShard::Green],
            generic: 0,
        })
        .id();

    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[
            (elf, AttackTarget::Player(P1)),
            (bear, AttackTarget::Player(P1)),
            (giant, AttackTarget::Player(P1)),
        ])
        .expect("declare attackers");
    advance_to_declare_blockers_and_give_priority(&mut runner, P1);

    // Provide mana for {2}{G} (X=2).
    if let Some(p) = runner.state_mut().players.iter_mut().find(|p| p.id == P1) {
        p.mana_pool = ManaPool::from_units(vec![
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]),
        ]);
    }

    // P1 casts Choking Vines targeting elf and bear (X=2).
    runner
        .cast(vines)
        .x(2)
        .target_objects(&[elf, bear])
        .resolve();

    // elf and bear are blocked; giant is still unblocked.
    assert!(!unblocked_attackers(runner.state()).contains(&elf));
    assert!(!unblocked_attackers(runner.state()).contains(&bear));
    assert!(unblocked_attackers(runner.state()).contains(&giant));

    // Elf took 1 damage and died (1 toughness).
    assert_eq!(
        runner.state().objects.get(&elf).map(|o| o.zone),
        Some(Zone::Graveyard),
        "the 1/1 elf dealt 1 lethal damage must be in the graveyard"
    );

    // Bear took 1 damage and survived (2 toughness, 1 damage marked).
    assert_eq!(
        runner.state().objects.get(&bear).map(|o| o.zone),
        Some(Zone::Battlefield),
        "the 2/2 bear must survive on battlefield"
    );
    assert_eq!(
        runner.state().objects.get(&bear).map(|o| o.damage_marked),
        Some(1),
        "the 2/2 bear must have 1 damage marked"
    );

    // Giant was un-targeted, so it took 0 damage.
    assert_eq!(
        runner.state().objects.get(&giant).map(|o| o.damage_marked),
        Some(0),
        "the un-targeted giant must have 0 damage marked"
    );
}

#[test]
fn winter_blast_parses_set_tap_state_then_damage_all() {
    let parsed = parse_oracle_text(
        WINTER_BLAST,
        "Winter Blast",
        &[],
        &["Sorcery".to_string()],
        &[],
    );
    assert_eq!(parsed.abilities.len(), 1, "one spell ability");
    let ability = &parsed.abilities[0];
    assert!(
        matches!(*ability.effect, Effect::SetTapState { .. }),
        "head effect must be SetTapState, got {:?}",
        ability.effect
    );
    let sub = ability
        .sub_ability
        .as_ref()
        .expect("damage sub-ability must compose after SetTapState");
    assert!(
        matches!(*sub.effect, Effect::DamageAll { .. }),
        "sub effect must be DamageAll, got {:?}",
        sub.effect
    );
}

#[test]
fn winter_blast_damages_only_tapped_creatures_with_flying() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let bird = scenario
        .add_creature(P1, "Flying Bird", 1, 1)
        .with_keyword(Keyword::Flying)
        .id();
    let bear = scenario.add_creature(P1, "Ground Bear", 2, 2).id();
    let blast = scenario
        .add_spell_to_hand_from_oracle(P0, "Winter Blast", false, WINTER_BLAST)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::X, ManaCostShard::Green],
            generic: 0,
        })
        .id();

    let mut runner = scenario.build();
    if let Some(p) = runner.state_mut().players.iter_mut().find(|p| p.id == P0) {
        p.mana_pool = ManaPool::from_units(vec![
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]),
        ]);
    }

    // P0 casts Winter Blast targeting bird and bear with X=2.
    runner
        .cast(blast)
        .x(2)
        .target_objects(&[bird, bear])
        .resolve();

    // Both creatures must be tapped.
    assert!(runner.state().objects.get(&bear).is_some_and(|o| o.tapped));

    // Bird has flying and 1 toughness; took 2 damage, so it died and is in the graveyard.
    assert_eq!(
        runner.state().objects.get(&bird).map(|o| o.zone),
        Some(Zone::Graveyard),
        "the 1/1 flying bird dealt 2 lethal damage by Winter Blast must be in the graveyard"
    );

    // Bear does not have flying; was tapped by the first effect, but took no damage from the second.
    assert_eq!(
        runner.state().objects.get(&bear).map(|o| o.zone),
        Some(Zone::Battlefield),
        "the ground bear must survive on the battlefield"
    );
    assert_eq!(
        runner.state().objects.get(&bear).map(|o| o.damage_marked),
        Some(0),
        "the ground bear must have 0 damage marked"
    );
}
