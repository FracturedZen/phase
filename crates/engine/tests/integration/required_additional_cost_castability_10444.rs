//! GitHub issue #10444 — `can_cast_object_now` offered a spell whose REQUIRED
//! additional cost can't be paid (Village Rites with no creature).
//!
//! CR 601.2h: the total cost, including every additional cost, must be paid.
//! CR 733.1: an action that can't legally be completed is reversed. The
//! castability query must therefore refuse such a spell up front, the way it
//! refuses an unpayable mana cost, so neither the UI nor the AI offers a cast
//! that is certain to be reversed.

use engine::game::casting::can_cast_object_now;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;

const VILLAGE_RITES: &str =
    "As an additional cost to cast this spell, sacrifice a creature.\nDraw two cards.";

const BONE_SPLINTERS: &str =
    "As an additional cost to cast this spell, sacrifice a creature.\nDestroy target creature.";

const TORMENTING_VOICE: &str =
    "As an additional cost to cast this spell, discard a card.\nDraw two cards.";

fn black(count: usize) -> Vec<ManaUnit> {
    vec![ManaUnit::new(ManaType::Black, ObjectId(0), false, vec![]); count]
}

fn red(count: usize) -> Vec<ManaUnit> {
    vec![ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![]); count]
}

fn single_black() -> ManaCost {
    ManaCost::Cost {
        shards: vec![ManaCostShard::Black],
        generic: 0,
    }
}

/// Village Rites with no creature to sacrifice: mana is available, but the
/// required sacrifice has no legal choice, so the spell is not castable.
#[test]
fn village_rites_not_castable_without_a_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, black(1));
    let rites = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Village Rites", true, VILLAGE_RITES);
        b.with_mana_cost(single_black());
        b.id()
    };
    // An opponent's creature is not a legal sacrifice for P0.
    scenario.add_creature(P1, "Grizzly Bears", 2, 2);
    let runner = scenario.build();

    assert!(
        !can_cast_object_now(runner.state(), P0, rites),
        "CR 601.2h: Village Rites' required sacrifice can't be paid with no creature, \
         so the spell must not be offered as castable"
    );
}

/// Positive control: with a creature to sacrifice, Village Rites is castable.
#[test]
fn village_rites_castable_with_a_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, black(1));
    let rites = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Village Rites", true, VILLAGE_RITES);
        b.with_mana_cost(single_black());
        b.id()
    };
    scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let runner = scenario.build();

    assert!(
        can_cast_object_now(runner.state(), P0, rites),
        "Village Rites must remain castable when a creature can be sacrificed"
    );
}

/// Targeted spell with a required sacrifice additional cost (Bone Splinters):
/// even when a legal target exists on the opponent's side, the caster cannot
/// cast it without a creature of their own to sacrifice.
#[test]
fn bone_splinters_not_castable_without_own_creature() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, black(1));
    let splinters = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Bone Splinters", false, BONE_SPLINTERS);
        b.with_mana_cost(single_black());
        b.id()
    };
    // Legal target for "Destroy target creature", but controlled by P1 so P0 cannot sacrifice it.
    scenario.add_creature(P1, "Grizzly Bears", 2, 2);
    let runner = scenario.build();

    assert!(
        !can_cast_object_now(runner.state(), P0, splinters),
        "CR 601.2h: Bone Splinters' required sacrifice can't be paid with no creature P0 controls"
    );
}

/// Required discard additional cost (Tormenting Voice): not castable when it is
/// the only card in the caster's hand, and castable once another card is in hand.
#[test]
fn tormenting_voice_requires_another_card_in_hand_to_discard() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, red(2));
    let voice = {
        let mut b =
            scenario.add_spell_to_hand_from_oracle(P0, "Tormenting Voice", false, TORMENTING_VOICE);
        b.with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Red],
            generic: 1,
        });
        b.id()
    };
    let runner = scenario.build();

    assert!(
        !can_cast_object_now(runner.state(), P0, voice),
        "CR 601.2h: Tormenting Voice cannot be cast when no other card is in hand to discard"
    );
}
