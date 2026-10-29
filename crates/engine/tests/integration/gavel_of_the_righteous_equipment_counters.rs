//! Gavel of the Righteous — the double-strike gate counts counters on the
//! Equipment, not on the equipped creature.
//!
//! Oracle text is the English Scryfall text (NCC #183), which prints
//! "this Equipment" rather than the card name:
//!
//!   Gavel of the Righteous
//!     At the beginning of combat on your turn, put a charge counter on this Equipment.
//!     Equipped creature gets +1/+1 for each counter on this Equipment.
//!     As long as this Equipment has four or more counters on it, equipped creature has double strike.
//!     Equip—Pay {3} or remove a counter from this Equipment.
//!
//! CR 201.5: "this Equipment" means the object the text is on, the Equipment.
//! CR 122.1: that clause counts counters on that object. The word "counters"
//! is untyped, so every counter on the Equipment counts.
//! CR 611.3a: the continuous effect applies to whatever its text indicates,
//! re-read as the board changes. CR 613.1f puts double strike in layer 6.
//! CR 702.4a defines double strike.
//!
//! Before the fix, the attached-subject rebind moved every `HasCounters` gate
//! onto the equipped creature. Four +1/+1 counters on the creature and three
//! on the Equipment granted double strike; four on the Equipment and none on
//! the creature did not.

use engine::game::game_object::AttachTarget;
use engine::game::keywords::has_keyword;
use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::counter::CounterType;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::phase::Phase;

const GAVEL_ORACLE: &str = "At the beginning of combat on your turn, put a charge counter on this Equipment.\nEquipped creature gets +1/+1 for each counter on this Equipment.\nAs long as this Equipment has four or more counters on it, equipped creature has double strike.\nEquip—Pay {3} or remove a counter from this Equipment.";

fn charge() -> CounterType {
    CounterType::Generic("charge".to_string())
}

fn set_counters(runner: &mut GameRunner, id: ObjectId, kind: CounterType, n: u32) {
    let obj = runner.state_mut().objects.get_mut(&id).unwrap();
    if n == 0 {
        obj.counters.remove(&kind);
    } else {
        obj.counters.insert(kind, n);
    }
}

/// Force a full layer re-derive, then read the effective keyword set (CR 613).
fn has_double_strike(runner: &mut GameRunner, id: ObjectId) -> bool {
    runner.state_mut().layers_dirty.mark_full();
    evaluate_layers(runner.state_mut());
    has_keyword(&runner.state().objects[&id], &Keyword::DoubleStrike)
}

/// CR 301.5: attach `equipment` to `host`.
fn attach(runner: &mut GameRunner, equipment: ObjectId, host: ObjectId) {
    let state = runner.state_mut();
    state.objects.get_mut(&equipment).unwrap().attached_to = Some(AttachTarget::Object(host));
    state
        .objects
        .get_mut(&host)
        .unwrap()
        .attachments
        .push(equipment);
}

/// CR 201.5 + CR 122.1 + CR 611.3a: double strike tracks the Equipment's
/// counters. The creature's own counters are a hostile fixture — the pre-fix
/// gate read those and ignored the Equipment.
#[test]
fn gavel_double_strike_counts_counters_on_the_equipment() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let bear = scenario.add_creature(P0, "Equipped Bear", 2, 2).id();
    let gavel = scenario
        .add_artifact_from_oracle(P0, "Gavel of the Righteous", GAVEL_ORACLE)
        .with_subtypes(vec!["Equipment"])
        .id();

    let mut runner = scenario.build();
    attach(&mut runner, gavel, bear);

    // Four counters on the Equipment, none on the creature: the grant is on.
    set_counters(&mut runner, gavel, charge(), 4);
    assert!(
        has_double_strike(&mut runner, bear),
        "CR 122.1 + CR 702.4a: four counters on the Equipment satisfy \"four or \
         more counters on it\". A gate that reads the creature stays off here."
    );

    // Three on the Equipment, four +1/+1 counters on the creature: the grant
    // is off. This is the revert discriminator.
    set_counters(&mut runner, gavel, charge(), 3);
    set_counters(&mut runner, bear, CounterType::Plus1Plus1, 4);
    assert!(
        !has_double_strike(&mut runner, bear),
        "CR 201.5: three counters on the Equipment do not satisfy the gate, \
         even when the equipped creature itself has four counters"
    );

    // Same board, Equipment back to four. The creature still has four +1/+1
    // counters, so a creature-bound gate would have been on already and this
    // step would not prove the Equipment is what changed.
    set_counters(&mut runner, gavel, charge(), 4);
    assert!(
        has_double_strike(&mut runner, bear),
        "raising the Equipment from three counters to four turns the grant on \
         while the creature's own counters stay at four"
    );
}
