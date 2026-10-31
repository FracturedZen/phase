//! Hot Pursuit (MKC, issue #7239): "At the beginning of combat on your turn, if
//! two or more players have lost the game, gain control of all goaded and/or
//! suspected creatures until end of turn. Untap them. They gain haste until end
//! of turn."
//!
//! Before the fix the whole trigger body was an `unparsed_condition` gap, so the
//! trigger never did anything. The fix parses the intervening-if (CR 603.4) as a
//! count of players who have lost the game (CR 104.3 + CR 800.4a), the
//! "goaded and/or suspected" designation disjunction (CR 701.15b + CR 701.60a),
//! and binds "them" to the creatures whose control was gained (CR 608.2c).

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::game::{layers, zones};
use engine::types::ability::{ContinuousModification, Duration, TargetFilter};
use engine::types::format::FormatConfig;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::statics::StaticMode;
use engine::types::zones::Zone;

const HOT_PURSUIT_ORACLE: &str = "When this enchantment enters, suspect target creature an opponent controls. As long as this enchantment remains on the battlefield, that creature is also goaded.\nAt the beginning of combat on your turn, if two or more players have lost the game, gain control of all goaded and/or suspected creatures until end of turn. Untap them. They gain haste until end of turn.";

const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

struct Board {
    runner: GameRunner,
    suspected: ObjectId,
    goaded: ObjectId,
    plain: ObjectId,
}

/// Five-player Commander game. P1 has a tapped suspected creature, P2 has a
/// tapped creature goaded by P1, and P1 also has an ordinary tapped creature.
/// `lost` of the players P3/P4 are marked as having lost the game.
fn board(lost: usize) -> Board {
    let mut scenario = GameScenario::new_with_format(FormatConfig::commander(), 5, 7239);
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_enchantment_from_oracle(P0, "Hot Pursuit", HOT_PURSUIT_ORACLE);
    let suspected = scenario.add_creature(P1, "Suspect", 2, 2).id();
    let goaded = scenario.add_creature(P2, "Goaded One", 3, 3).id();
    let plain = scenario.add_creature(P1, "Bystander", 1, 1).id();
    let mut runner = scenario.build();
    let state = runner.state_mut();
    for id in [suspected, goaded, plain] {
        state.objects.get_mut(&id).unwrap().tapped = true;
    }
    state.objects.get_mut(&suspected).unwrap().is_suspected = true;
    state.objects.get_mut(&goaded).unwrap().goaded_by.insert(P1);
    for seat in 3..3 + lost {
        state.players[seat].is_eliminated = true;
    }
    Board {
        runner,
        suspected,
        goaded,
        plain,
    }
}

fn run_to_combat_trigger(runner: &mut GameRunner) {
    runner.advance_to_phase(Phase::BeginCombat);
    runner.advance_until_stack_empty();
}

#[test]
fn hot_pursuit_two_losses_steals_untaps_and_hastes_goaded_and_suspected() {
    let Board {
        mut runner,
        suspected,
        goaded,
        plain,
    } = board(2);
    run_to_combat_trigger(&mut runner);
    let state = runner.state();
    for (id, label) in [(suspected, "suspected"), (goaded, "goaded")] {
        let obj = &state.objects[&id];
        assert_eq!(obj.controller, P0, "{label} creature: control gained");
        assert!(
            !obj.tapped,
            "{label} creature: untapped (them = gained set)"
        );
        assert!(
            obj.has_keyword(&Keyword::Haste),
            "{label} creature: gains haste"
        );
    }
    let bystander = &state.objects[&plain];
    assert_eq!(
        bystander.controller, P1,
        "neither goaded nor suspected: untouched"
    );
    assert!(
        bystander.tapped,
        "neither goaded nor suspected: stays tapped"
    );
}

#[test]
fn hot_pursuit_one_loss_does_nothing() {
    let Board {
        mut runner,
        suspected,
        goaded,
        ..
    } = board(1);
    run_to_combat_trigger(&mut runner);
    let state = runner.state();
    assert_eq!(state.objects[&suspected].controller, P1);
    assert_eq!(state.objects[&goaded].controller, P2);
    assert!(state.objects[&suspected].tapped);
}

/// The transient goad designations on `id` whose goader is `player`
/// (CR 701.15b: the controller of the installing effect is the goading player).
fn goad_effects_on(runner: &GameRunner, id: ObjectId, player: PlayerId) -> usize {
    runner
        .state()
        .transient_continuous_effects
        .iter()
        .filter(|tce| {
            matches!(tce.affected, TargetFilter::SpecificObject { id: a } if a == id)
                && tce.controller == player
                && tce.duration == Duration::WhileHostOnBattlefield
                && tce.modifications.iter().any(|m| {
                    matches!(
                        m,
                        ContinuousModification::AddStaticMode {
                            mode: StaticMode::Goaded
                        }
                    )
                })
        })
        .count()
}

/// Cast Hot Pursuit and resolve its ETB against P1's only creature.
fn cast_and_resolve_etb() -> (GameRunner, ObjectId, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let foe = scenario.add_creature(P1, "Foe", 2, 2).id();
    let pursuit = scenario
        .add_artifact_to_hand_from_oracle(P0, "Hot Pursuit", HOT_PURSUIT_ORACLE)
        .id();
    let mut runner = scenario.build();
    runner.cast(pursuit).resolve();
    for _ in 0..8 {
        if matches!(
            runner.state().waiting_for,
            WaitingFor::TriggerTargetSelection { .. } | WaitingFor::TargetSelection { .. }
        ) {
            runner.choose_first_legal_target().expect("target the foe");
        } else {
            break;
        }
    }
    runner.advance_until_stack_empty();
    (runner, pursuit, foe)
}

/// CR 701.60a + CR 701.15b + CR 611.2b: the ETB suspects the target and goads it
/// (P0 is the goader) for as long as Hot Pursuit remains on the battlefield.
#[test]
fn hot_pursuit_etb_suspects_and_goads_while_on_battlefield() {
    let (runner, pursuit, foe) = cast_and_resolve_etb();
    let state = runner.state();
    assert_eq!(state.objects[&pursuit].zone, Zone::Battlefield);
    assert!(state.objects[&foe].is_suspected, "target suspected");
    assert_eq!(goad_effects_on(&runner, foe, P0), 1, "target goaded by P0");
}

/// CR 611.2b: once Hot Pursuit leaves the battlefield the goad ends; the
/// suspected designation (CR 701.60c) stays.
#[test]
fn hot_pursuit_goad_ends_when_it_leaves() {
    let (mut runner, pursuit, foe) = cast_and_resolve_etb();
    assert_eq!(goad_effects_on(&runner, foe, P0), 1, "reach guard: goaded");
    let mut events = Vec::new();
    zones::move_to_zone(runner.state_mut(), pursuit, Zone::Graveyard, &mut events);
    layers::flush_layers(runner.state_mut());
    assert_eq!(goad_effects_on(&runner, foe, P0), 0, "goad ended");
    assert!(runner.state().objects[&foe].is_suspected, "still suspected");
}
