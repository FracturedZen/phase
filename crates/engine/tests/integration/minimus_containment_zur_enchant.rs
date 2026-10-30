//! Minimus Containment fetched by Zur must offer a legal nonland host.
//!
//! CR 702.5a: "Enchant nonland permanent" is the Aura's enchant ability.
//! CR 303.4f: Zur puts the Aura onto the battlefield, so its controller chooses
//! what it enchants. CR 704.5m: an Aura that enters with no enchant ability is
//! unattached and is put into its owner's graveyard.

use engine::game::combat::AttackTarget;
use engine::game::game_object::AttachTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::{TargetFilter, TargetRef, TypeFilter};
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::mana::{ManaCost, ManaCostShard};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const ZUR_ORACLE: &str = "Flying\nWhenever Zur attacks, you may search your library for an enchantment card with mana value 3 or less, put it onto the battlefield, then shuffle.";
const MINIMUS_ORACLE: &str = "Enchant nonland permanent\nEnchanted permanent is a Treasure artifact with \"{T}, Sacrifice this artifact: Add one mana of any color,\" and it loses all other abilities. (If it was a creature, it's no longer a creature.)";

#[test]
fn zur_puts_minimus_containment_onto_a_chosen_nonland() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let zur = scenario
        .add_creature_from_oracle(P0, "Zur the Enchanter", 1, 4, ZUR_ORACLE)
        .id();
    let bear = scenario.add_vanilla(P0, 2, 2);
    let bird = scenario.add_vanilla(P0, 1, 1);
    let forest = scenario.add_land_from_oracle(P0, "Forest", "").id();

    let minimus = scenario
        .add_spell_to_library_top(P0, "Minimus Containment", false)
        .as_enchantment()
        .with_subtypes(vec!["Aura"])
        .with_mana_cost(ManaCost::Cost {
            generic: 2,
            shards: vec![ManaCostShard::White],
        })
        // MTGJSON reports the keyword name; the Oracle line carries the
        // restriction. That is the production pair this bug drops.
        .from_oracle_text_with_keywords(&["enchant"], MINIMUS_ORACLE)
        .id();
    let too_expensive = scenario
        .add_spell_to_library_top(P0, "Costly Aura", false)
        .as_enchantment()
        .with_subtypes(vec!["Aura"])
        .with_mana_cost(ManaCost::Cost {
            generic: 4,
            shards: vec![],
        })
        .id();

    let mut runner = scenario.build();
    let enchant = runner.state().objects[&minimus]
        .keywords
        .iter()
        .find_map(|keyword| match keyword {
            Keyword::Enchant(filter) => Some(filter.clone()),
            _ => None,
        })
        .expect("the printed Enchant line must be a keyword on the library card");
    let TargetFilter::Typed(typed) = enchant else {
        panic!("expected a typed enchant restriction, got {enchant:?}");
    };
    assert_eq!(
        typed.type_filters,
        vec![
            TypeFilter::Permanent,
            TypeFilter::Non(Box::new(TypeFilter::Land)),
        ]
    );
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(zur, AttackTarget::Player(P1))])
        .expect("Zur can be declared as an attacker");

    let legal_hosts = drive_to_aura_host_choice(&mut runner, minimus, too_expensive);
    let host_ids: Vec<ObjectId> = legal_hosts
        .iter()
        .filter_map(|target| match target {
            TargetRef::Object(id) => Some(*id),
            TargetRef::Player(_) => None,
        })
        .collect();
    assert!(
        host_ids.contains(&bear) && host_ids.contains(&bird),
        "both nonland permanents must be legal hosts, got {legal_hosts:?}"
    );
    assert!(
        !host_ids.contains(&forest),
        "a land is not a legal host for Enchant nonland permanent, got {legal_hosts:?}"
    );

    runner
        .act(GameAction::ChooseTarget {
            target: Some(TargetRef::Object(bear)),
        })
        .expect("choosing a nonland host must attach Minimus");

    assert_eq!(runner.state().objects[&minimus].zone, Zone::Battlefield);
    assert_eq!(
        runner.state().objects[&minimus].attached_to,
        Some(AttachTarget::Object(bear))
    );
}

/// Walk Zur's attack trigger through the optional search and stop at the
/// CR 303.4f host choice. Returning that choice's legal targets is the
/// reach-guard: the search found Minimus and the enchant ability is live.
fn drive_to_aura_host_choice(
    runner: &mut GameRunner,
    minimus: ObjectId,
    too_expensive: ObjectId,
) -> Vec<TargetRef> {
    for _ in 0..24 {
        match runner.state().waiting_for.clone() {
            WaitingFor::OrderTriggers { .. } => {
                engine::game::triggers::drain_order_triggers_with_identity(runner.state_mut());
            }
            WaitingFor::Priority { .. } => {
                assert!(
                    !runner.state().stack.is_empty(),
                    "Minimus is in {:?} before a host was chosen",
                    runner.state().objects[&minimus].zone
                );
                runner
                    .act(GameAction::PassPriority)
                    .expect("passing priority while Zur's trigger is on the stack");
            }
            WaitingFor::OptionalEffectChoice { .. } => {
                runner
                    .act(GameAction::DecideOptionalEffect { accept: true })
                    .expect("Zur's search is optional and the test accepts it");
            }
            WaitingFor::SearchChoice { player, cards, .. } => {
                assert_eq!(player, P0, "Zur's controller searches");
                assert!(
                    cards.contains(&minimus),
                    "mana value 3 Minimus must be a legal search choice, got {cards:?}"
                );
                assert!(
                    !cards.contains(&too_expensive),
                    "mana value 4 enchantment must be outside Zur's search, got {cards:?}"
                );
                runner
                    .act(GameAction::SelectCards {
                        cards: vec![minimus],
                    })
                    .expect("selecting Minimus must put it onto the battlefield");
            }
            WaitingFor::ReturnAsAuraTarget {
                player,
                legal_targets,
                ..
            } => {
                assert_eq!(player, P0);
                return legal_targets;
            }
            other => panic!(
                "expected Zur's search to reach an Aura host choice, got {other:?}; Minimus is in {:?}",
                runner.state().objects[&minimus].zone
            ),
        }
    }
    panic!(
        "Zur's trigger never offered an Aura host; Minimus is in {:?}",
        runner.state().objects[&minimus].zone
    );
}
