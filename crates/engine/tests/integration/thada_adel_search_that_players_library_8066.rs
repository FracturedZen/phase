//! Issue #8066 — Thada Adel, Acquisitor searched its controller's library.
//!
//! Oracle:
//! > Islandwalk
//! > Whenever Thada Adel deals combat damage to a player, search that player's
//! > library for an artifact card and exile it. Then that player shuffles.
//! > Until end of turn, you may play that card.
//!
//! CR 603.2 + CR 608.2c: "that player" is the player the combat damage was
//! dealt to. CR 701.23a: the trigger's controller searches that player's
//! library (the searched library and the searcher differ).

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{ControllerRef, Effect, TargetFilter};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

use super::rules::run_combat;

const THADA_ORACLE: &str = "Islandwalk\nWhenever Thada Adel deals combat damage to a player, search that player's library for an artifact card and exile it. Then that player shuffles. Until end of turn, you may play that card.";
const ROOTWATER_ORACLE: &str = "Whenever this creature deals combat damage to a player, you may pay {2}. If you do, search that player's library for a card and exile it, then the player shuffles.";

fn search_target_player(ability: &engine::types::ability::AbilityDefinition) -> Option<TargetFilter> {
    let mut cursor = Some(ability);
    while let Some(node) = cursor {
        if let Effect::SearchLibrary { target_player, .. } = node.effect.as_ref() {
            return target_player.clone();
        }
        cursor = node.sub_ability.as_deref();
    }
    panic!("no SearchLibrary in chain: {ability:?}");
}

fn triggering_player_library() -> Option<TargetFilter> {
    Some(TargetFilter::Typed(
        engine::types::ability::TypedFilter::default().controller(ControllerRef::TriggeringPlayer),
    ))
}

fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn combat_damage_that_players_library_binds_the_damaged_player() {
    on_big_stack(|| {
        for (name, oracle) in [
            ("Thada Adel, Acquisitor", THADA_ORACLE),
            ("Rootwater Thief", ROOTWATER_ORACLE),
        ] {
            let parsed = parse_oracle_text(oracle, name, &[], &["Creature".to_string()], &[]);
            let execute = parsed.triggers[0].execute.as_deref().expect("trigger body");
            assert_eq!(
                search_target_player(execute),
                triggering_player_library(),
                "{name}: \"search that player's library\" must name the damaged player"
            );
        }
    });
}

fn make_artifact(runner: &mut GameRunner, id: ObjectId) {
    let obj = runner.state_mut().objects.get_mut(&id).unwrap();
    obj.card_types.core_types = vec![CoreType::Artifact];
    obj.base_card_types.core_types = vec![CoreType::Artifact];
}

#[test]
fn thada_adel_exiles_an_artifact_from_the_damaged_players_library() {
    on_big_stack(|| {
        let mut scenario = GameScenario::new();
        scenario.at_phase(Phase::PreCombatMain);
        let thada = scenario
            .add_creature_from_oracle(P0, "Thada Adel, Acquisitor", 1, 3, THADA_ORACLE)
            .id();
        let own_artifact = scenario.add_card_to_library_top(P0, "Controller's Relic");
        let their_artifact = scenario.add_card_to_library_top(P1, "Opponent's Relic");
        let their_nonartifact = scenario.add_card_to_library_top(P1, "Opponent's Spell");
        let mut runner = scenario.build();
        make_artifact(&mut runner, own_artifact);
        make_artifact(&mut runner, their_artifact);
        runner.state_mut().layers_dirty.mark_full();

        run_combat(&mut runner, vec![thada], vec![]);

        let mut searched = false;
        for _ in 0..50 {
            match runner.state().waiting_for.clone() {
                WaitingFor::SearchChoice { player, cards, .. } => {
                    assert_eq!(player, P0, "Thada's controller is the searcher (CR 701.23a)");
                    assert!(
                        cards.contains(&their_artifact),
                        "the damaged player's artifact must be a legal choice; got {cards:?}"
                    );
                    assert!(
                        !cards.contains(&own_artifact),
                        "the controller's own library must not be searched; got {cards:?}"
                    );
                    assert!(
                        !cards.contains(&their_nonartifact),
                        "only artifact cards are legal choices; got {cards:?}"
                    );
                    runner
                        .act(GameAction::SelectCards {
                            cards: vec![their_artifact],
                        })
                        .expect("select the found artifact");
                    searched = true;
                }
                WaitingFor::Priority { .. } if !runner.state().stack.is_empty() || !searched => {
                    if runner.act(GameAction::PassPriority).is_err() {
                        break;
                    }
                }
                _ => break,
            }
            if searched && runner.state().stack.is_empty() {
                break;
            }
        }
        assert!(searched, "Thada's trigger must reach a search choice");
        assert_eq!(
            runner.state().objects[&their_artifact].zone,
            Zone::Exile,
            "the found artifact is exiled from the damaged player's library"
        );
        assert_eq!(
            runner.state().objects[&own_artifact].zone,
            Zone::Library,
            "the controller's artifact stays in their library"
        );
        assert!(
            runner.state().players[P1.0 as usize]
                .library
                .contains(&their_nonartifact),
            "the rest of the damaged player's library stays put"
        );
    });
}
