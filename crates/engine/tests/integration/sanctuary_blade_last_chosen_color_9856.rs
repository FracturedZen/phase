//! #9856 / F9 — Sanctuary Blade: "As this Equipment becomes attached to a
//! creature, choose a color. Equipped creature gets +2/+0 and has protection
//! from the last chosen color."
//!
//! Chosen colours ACCUMULATE on a source (`apply_choice_attributes`), and the
//! CR 607.2d linked read (`GameObject::chosen_color`) returns the OLDEST one.
//! "The last chosen color" is not a linked "the chosen color" anaphor: it names
//! the most recent answer, so after the Blade is re-equipped and a new colour
//! is chosen, the equipped creature must have protection from the NEW colour
//! only. `ProtectionTarget::LastChosenColor` resolves via
//! `GameObject::current_chosen_color` (newest).
//!
//! REVERT DISCRIMINATOR: map "the last chosen color" back to
//! `ProtectionTarget::ChosenColor` and the second equip still grants
//! protection from Blue (the first answer).

use engine::game::game_object::AttachTarget;
use engine::game::scenario::{GameScenario, P0};
use engine::types::ability::ChoiceType;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::{Keyword, ProtectionTarget};
use engine::types::mana::{ManaColor, ManaType, ManaUnit};
use engine::types::phase::Phase;

/// Sanctuary Blade {2} Artifact — Equipment — verbatim Oracle text.
const SANCTUARY_BLADE: &str = "As this Equipment becomes attached to a creature, choose a color.\nEquipped creature gets +2/+0 and has protection from the last chosen color.\nEquip {3}";

fn colorless(n: usize) -> Vec<ManaUnit> {
    (0..n)
        .map(|_| ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]))
        .collect()
}

fn equip_and_choose(
    runner: &mut engine::game::scenario::GameRunner,
    blade: ObjectId,
    target: ObjectId,
    color: &str,
) {
    let equip_idx = runner.state().objects[&blade]
        .abilities
        .iter()
        .position(|a| {
            a.description
                .as_deref()
                .is_some_and(|d| d.contains("Equip"))
        })
        .expect("Sanctuary Blade must carry an Equip activated ability");
    runner
        .activate(blade, equip_idx)
        .target_object(target)
        .resolve();
    match &runner.state().waiting_for {
        WaitingFor::NamedChoice {
            choice_type: ChoiceType::Color { .. },
            ..
        } => {}
        other => panic!("expected a colour choice as the Blade attaches, got {other:?}"),
    }
    runner
        .act(GameAction::ChooseOption {
            choice: color.to_string(),
        })
        .expect("colour choice must be accepted");
    runner.advance_until_stack_empty();
    assert!(
        matches!(
            runner.state().objects[&blade].attached_to,
            Some(AttachTarget::Object(id)) if id == target
        ),
        "Sanctuary Blade must be attached to the equip target"
    );
}

fn has_protection_from(
    runner: &engine::game::scenario::GameRunner,
    id: ObjectId,
    c: ManaColor,
) -> bool {
    runner.state().objects[&id]
        .keywords
        .iter()
        .any(|k| *k == Keyword::Protection(ProtectionTarget::Color(c)))
}

#[test]
fn sanctuary_blade_reequip_uses_the_last_chosen_color() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.with_mana_pool(P0, colorless(6));

    let bear = scenario.add_creature(P0, "Grizzly Bears", 2, 2).id();
    let elf = scenario.add_creature(P0, "Llanowar Elves", 1, 1).id();
    let blade = scenario
        .add_creature(P0, "Sanctuary Blade", 0, 0)
        .as_artifact()
        .with_subtypes(vec!["Equipment"])
        .from_oracle_text(SANCTUARY_BLADE)
        .id();

    let mut runner = scenario.build();

    // First attach: Blue. Reach guard — the grant works at all.
    equip_and_choose(&mut runner, blade, bear, "Blue");
    assert!(
        has_protection_from(&runner, bear, ManaColor::Blue),
        "first equip must grant protection from the chosen colour (Blue)"
    );

    // Re-equip to another creature and choose Red.
    equip_and_choose(&mut runner, blade, elf, "Red");
    assert!(
        has_protection_from(&runner, elf, ManaColor::Red),
        "after re-equipping and choosing Red, the equipped creature must have \
         protection from the LAST chosen colour (Red)"
    );
    assert!(
        !has_protection_from(&runner, elf, ManaColor::Blue),
        "the first, superseded choice (Blue) must no longer be granted"
    );
    assert!(
        !has_protection_from(&runner, bear, ManaColor::Blue),
        "the previously equipped creature must lose the Blade's protection"
    );
}
