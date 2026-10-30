//! Issue #8063 — Kari Zev's Expertise: after "Gain control of target creature
//! or Vehicle until end of turn. Untap it. It gains haste until end of turn.",
//! the free cast "You may cast a spell with mana value 2 or less from your hand
//! without paying its mana cost" never offered a card.
//!
//! CR 608.2g + CR 115.10a: the free cast picks a card from the hand while the
//! effect resolves. It is not a target and must not inherit the creature the
//! earlier instruction targeted; inheriting it made the cast resolver see a
//! battlefield object as its only "candidate", so it skipped the hand pick.

use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::ability::Effect;
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaCost, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const KARI_ZEV_EXPERTISE: &str = "Gain control of target creature or Vehicle until end of turn. \
Untap it. It gains haste until end of turn.\n\
You may cast a spell with mana value 2 or less from your hand without paying its mana cost.";

const TAP_THEN_FREE_CAST: &str = "Tap target creature.\n\
You may cast a spell with mana value 2 or less from your hand without paying its mana cost.";

fn colorless(n: usize) -> Vec<ManaUnit> {
    (0..n)
        .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
        .collect()
}

fn chain_has_free_cast(effect_chain: &engine::types::ability::AbilityDefinition) -> bool {
    let mut node = Some(effect_chain);
    while let Some(def) = node {
        if matches!(
            def.effect.as_ref(),
            Effect::CastFromZone {
                without_paying_mana_cost: true,
                ..
            }
        ) {
            return true;
        }
        node = def.sub_ability.as_deref();
    }
    false
}

/// Casts `oracle` targeting a P1 creature, accepts the optional free cast and
/// returns (runner, stolen creature, free spell, too-expensive spell).
fn cast_and_accept(
    name: &str,
    oracle: &str,
) -> (
    engine::game::scenario::GameRunner,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let victim = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();
    let sorcery = scenario
        .add_spell_to_hand_from_oracle(P0, name, false, oracle)
        .with_mana_cost(ManaCost::generic(3))
        .id();
    let cheap = scenario
        .add_spell_to_hand(P0, "Cheap Draw", true)
        .with_mana_cost(ManaCost::generic(2))
        .from_oracle_text("Draw a card.")
        .id();
    let pricey = scenario
        .add_spell_to_hand(P0, "Pricey Draw", true)
        .with_mana_cost(ManaCost::generic(3))
        .from_oracle_text("Draw a card.")
        .id();
    scenario.with_library_top(P0, &["Library One", "Library Two"]);
    scenario.with_mana_pool(P0, colorless(3));

    let mut runner = scenario.build();
    assert!(
        chain_has_free_cast(&runner.state().objects[&sorcery].abilities[0]),
        "the free-cast clause must be part of the spell's chain"
    );
    let card_id = runner.state().objects[&sorcery].card_id;
    runner
        .act(GameAction::CastSpell {
            object_id: sorcery,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("cast the sorcery");
    runner.advance_until_stack_empty();

    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::OptionalEffectChoice { player, .. } if player == P0
        ),
        "expected the optional free-cast prompt, got {:?}",
        runner.state().waiting_for
    );
    runner
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accept the free cast");
    (runner, victim, cheap, pricey)
}

#[test]
fn kari_zev_expertise_offers_hand_card_after_gaining_control() {
    let (mut runner, victim, cheap, pricey) =
        cast_and_accept("Kari Zev's Expertise", KARI_ZEV_EXPERTISE);

    // The control-gain half still applies to the targeted creature.
    let stolen = &runner.state().objects[&victim];
    assert_eq!(
        stolen.controller, P0,
        "Kari Zev's Expertise steals the creature"
    );
    assert!(!stolen.tapped, "the stolen creature is untapped");
    assert!(
        stolen.has_keyword(&Keyword::Haste),
        "the stolen creature gains haste"
    );

    match &runner.state().waiting_for {
        WaitingFor::EffectZoneChoice { cards, .. } => {
            assert!(cards.contains(&cheap), "MV 2 spell in hand is offered");
            assert!(!cards.contains(&pricey), "MV 3 spell is not offered");
            assert!(
                !cards.contains(&victim),
                "the targeted creature is not a free-cast candidate"
            );
        }
        other => panic!("expected a hand pick for the free cast, got {other:?}"),
    }

    runner
        .act(GameAction::SelectCards { cards: vec![cheap] })
        .expect("pick the free spell");
    assert_eq!(
        runner.state().objects[&cheap].zone,
        Zone::Stack,
        "the picked spell is cast during resolution"
    );
    runner.advance_until_stack_empty();
    assert_eq!(runner.state().objects[&cheap].zone, Zone::Graveyard);
    assert_eq!(runner.state().objects[&pricey].zone, Zone::Hand);
    assert_eq!(runner.state().objects[&victim].controller, P0);
}

#[test]
fn targeted_instruction_then_free_cast_from_hand_offers_hand_card() {
    let (mut runner, victim, cheap, _pricey) = cast_and_accept("Tap Then Cast", TAP_THEN_FREE_CAST);
    assert!(runner.state().objects[&victim].tapped);
    match &runner.state().waiting_for {
        WaitingFor::EffectZoneChoice { cards, .. } => {
            assert_eq!(cards, &vec![cheap], "only the MV 2 hand spell is offered");
        }
        other => panic!("expected a hand pick for the free cast, got {other:?}"),
    }
    runner
        .act(GameAction::SelectCards { cards: vec![cheap] })
        .expect("pick the free spell");
    assert_eq!(runner.state().objects[&cheap].zone, Zone::Stack);
}
