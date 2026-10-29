//! Runtime tests for "is exiled from the battlefield while you're activating a
//! craft ability" (Market Gnome, issue #10195).
//!
//! CR 702.167a: Craft's cost includes "Exile [materials] from among permanents
//! you control and/or cards in your graveyard". A permanent exiled to pay that
//! cost is exiled while its controller is activating a craft ability, so Market
//! Gnome's trigger fires. Exiling it any other way (Swords to Plowshares) does
//! not satisfy the "while you're activating a craft ability" qualifier.

use crate::game::scenario::{GameScenario, P0};
use crate::types::ability::{
    AbilityCost, AbilityDefinition, AbilityKind, ControllerRef, CostObjectCount, Effect,
    FilterProp, TargetFilter, TypeFilter, TypedFilter,
};
use crate::types::actions::GameAction;
use crate::types::game_state::{CastPaymentMode, PayCostKind, WaitingFor};
use crate::types::identifiers::ObjectId;
use crate::types::keywords::Keyword;
use crate::types::mana::{ManaCost, ManaType, ManaUnit};
use crate::types::phase::Phase;
use crate::types::zones::Zone;

const MARKET_GNOME: &str = "When this creature dies, you gain 1 life and draw a card.\n\
When this creature is exiled from the battlefield while you're activating a craft ability, \
you gain 1 life and draw a card.";

const SWORDS: &str = "Exile target creature. Its controller gains life equal to its power.";

/// CR 702.167b: "craft with creature" materials — a creature you control or a
/// creature card in your graveyard.
fn creature_materials() -> TargetFilter {
    TargetFilter::Or {
        filters: vec![
            TargetFilter::Typed(
                TypedFilter::permanent()
                    .with_type(TypeFilter::Creature)
                    .controller(ControllerRef::You)
                    .properties(vec![FilterProp::InZone {
                        zone: Zone::Battlefield,
                    }]),
            ),
            TargetFilter::Typed(
                TypedFilter::card()
                    .with_type(TypeFilter::Creature)
                    .properties(vec![
                        FilterProp::InZone {
                            zone: Zone::Graveyard,
                        },
                        FilterProp::Owned {
                            controller: ControllerRef::You,
                        },
                    ]),
            ),
        ],
    }
}

fn one_generic() -> ManaCost {
    ManaCost::Cost {
        shards: vec![],
        generic: 1,
    }
}

/// The ability `synthesize_craft` builds for "Craft with creature {1}".
fn craft_ability() -> AbilityDefinition {
    AbilityDefinition::new(
        AbilityKind::Activated,
        Effect::ChangeZone {
            origin: Some(Zone::Exile),
            destination: Zone::Battlefield,
            target: TargetFilter::SelfRef,
            owner_library: false,
            enter_transformed: true,
            enters_under: None,
            enter_tapped: crate::types::zones::EtbTapState::Unspecified,
            enters_attacking: false,
            up_to: false,
            enter_with_counters: Vec::new(),
            conditional_enter_with_counters: vec![],
            face_down_profile: None,
            enters_modified_if: None,
        },
    )
    .cost(AbilityCost::Composite {
        costs: vec![
            AbilityCost::Mana {
                cost: one_generic(),
            },
            AbilityCost::Exile {
                count: 1,
                zone: Some(Zone::Battlefield),
                filter: Some(TargetFilter::SelfRef),
            },
            AbilityCost::ExileMaterials {
                materials: creature_materials(),
                count: CostObjectCount::exactly(1),
            },
        ],
    })
    .sorcery_speed()
}

fn hand_count(runner: &crate::game::scenario::GameRunner) -> usize {
    runner
        .state()
        .players
        .iter()
        .find(|p| p.id == P0)
        .expect("P0 exists")
        .hand
        .len()
}

fn scenario_with_gnome() -> (GameScenario, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_life(P0, 20);
    scenario.with_library_top(P0, &["Library Card A", "Library Card B"]);
    let gnome = scenario
        .add_creature_from_oracle(P0, "Market Gnome", 0, 3, MARKET_GNOME)
        .id();
    (scenario, gnome)
}

/// CR 702.167a + CR 603.6c: Market Gnome exiled as a craft material triggers —
/// its controller gains 1 life and draws a card.
#[test]
fn market_gnome_exiled_as_craft_material_gains_life_and_draws() {
    let (mut scenario, gnome) = scenario_with_gnome();
    let crafter = scenario
        .add_creature(P0, "Test Crafter", 0, 0)
        .as_artifact()
        .with_keyword(Keyword::Craft {
            cost: one_generic(),
            materials: creature_materials(),
            count: CostObjectCount::exactly(1),
        })
        .with_ability_definition(craft_ability())
        .id();
    let mut runner = scenario.build();
    runner
        .state_mut()
        .players
        .iter_mut()
        .find(|p| p.id == P0)
        .unwrap()
        .mana_pool
        .add(ManaUnit::new(
            ManaType::Colorless,
            ObjectId(0),
            false,
            vec![],
        ));
    let life_before = runner.life(P0);
    let hand_before = hand_count(&runner);

    let result = runner
        .act(GameAction::ActivateAbility {
            source_id: crafter,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("craft activation at sorcery speed is legal");
    assert!(
        matches!(
            result.waiting_for,
            WaitingFor::PayCost {
                kind: PayCostKind::ExileMaterials { .. },
                ref choices,
                ..
            } if choices.contains(&gnome)
        ),
        "Market Gnome must be offered as a craft material, got {:?}",
        result.waiting_for
    );
    runner
        .act(GameAction::SelectCards { cards: vec![gnome] })
        .expect("selecting Market Gnome as the material is accepted");
    runner.advance_until_stack_empty();

    assert_eq!(runner.state().objects[&gnome].zone, Zone::Exile);
    assert_eq!(
        runner.life(P0) - life_before,
        1,
        "CR 702.167a: exiled while activating a craft ability — gain 1 life"
    );
    assert_eq!(
        hand_count(&runner) - hand_before,
        1,
        "CR 702.167a: exiled while activating a craft ability — draw a card"
    );
}

/// CR 603.6c: Exiling Market Gnome with a spell is not a craft activation, so
/// the craft-qualified trigger does not fire (issue #10195: it used to fire on
/// any exile). Swords' own life gain is the Gnome's power, 0.
#[test]
fn market_gnome_exiled_by_swords_does_not_trigger() {
    let (mut scenario, gnome) = scenario_with_gnome();
    let swords = scenario
        .add_spell_to_hand_from_oracle(P0, "Swords to Plowshares", true, SWORDS)
        .with_mana_cost(ManaCost::zero())
        .id();
    let mut runner = scenario.build();
    let life_before = runner.life(P0);
    let hand_before = hand_count(&runner);

    runner.cast(swords).target_object(gnome).resolve();
    runner.advance_until_stack_empty();

    assert_eq!(runner.state().objects[&gnome].zone, Zone::Exile);
    assert_eq!(
        runner.life(P0),
        life_before,
        "no craft activation — Market Gnome's exile trigger must not fire"
    );
    // Swords left the hand; no card was drawn.
    assert_eq!(hand_count(&runner), hand_before - 1);
}
