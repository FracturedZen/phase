//! Issue #8154 — Brudiclad, Telchor Engineer: "At the beginning of combat on
//! your turn, create a 2/1 blue Phyrexian Myr artifact creature token. Then
//! you may choose a token you control. If you do, each other token you control
//! becomes a copy of that token."
//!
//! CR 608.2c + CR 707.2: the choice is made during resolution, after the Myr
//! exists (so the new Myr may be chosen or copied over), and the copy effect
//! applies to each OTHER token — never to Brudiclad, which is not a token.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;

const BRUDICLAD: &str = "Creature tokens you control have haste.\nAt the beginning of combat on your turn, create a 2/1 blue Phyrexian Myr artifact creature token. Then you may choose a token you control. If you do, each other token you control becomes a copy of that token.";

struct Setup {
    runner: GameRunner,
    brudiclad: ObjectId,
    goblin: ObjectId,
    soldier: ObjectId,
}

fn setup() -> Setup {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let brudiclad = scenario
        .add_creature(P0, "Brudiclad, Telchor Engineer", 4, 4)
        .as_legendary()
        .from_oracle_text(BRUDICLAD)
        .id();
    let goblin = scenario.add_creature(P0, "Goblin", 1, 1).id();
    let soldier = scenario.add_creature(P0, "Soldier", 1, 1).id();
    let mut runner = scenario.build();
    for id in [goblin, soldier] {
        runner.state_mut().objects.get_mut(&id).unwrap().is_token = true;
    }
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };
    Setup {
        runner,
        brudiclad,
        goblin,
        soldier,
    }
}

/// Which token to answer the resolution-time choice with.
#[derive(Clone, Copy)]
enum Pick {
    Decline,
    Existing(ObjectId),
    /// The Myr created by this same resolution: it does not exist when the
    /// trigger is put on the stack, so it is identified from the offered set.
    NewMyr,
}

/// Drive the beginning-of-combat trigger, answering the token choice with
/// `pick`. Returns whether any target prompt appeared (it must not: the
/// choice is not a target, CR 115.10a).
fn run_trigger(s: &mut Setup, pick: Pick) -> bool {
    let (goblin, soldier) = (s.goblin, s.soldier);
    let chosen = |offered: &[ObjectId]| -> Vec<ObjectId> {
        match pick {
            Pick::Decline => vec![],
            Pick::Existing(id) => vec![id],
            Pick::NewMyr => offered
                .iter()
                .copied()
                .filter(|id| *id != goblin && *id != soldier)
                .collect(),
        }
    };
    let mut targeted = false;
    s.runner.advance_to_phase(Phase::BeginCombat);
    for _ in 0..40 {
        let waiting = s.runner.state().waiting_for.clone();
        let result = match waiting {
            WaitingFor::Priority { .. } if s.runner.state().stack.is_empty() => break,
            WaitingFor::Priority { .. } => s.runner.act(GameAction::PassPriority),
            WaitingFor::TargetSelection { .. } | WaitingFor::TriggerTargetSelection { .. } => {
                targeted = true;
                s.runner.act(GameAction::SelectTargets {
                    targets: chosen(&[]).into_iter().map(TargetRef::Object).collect(),
                })
            }
            WaitingFor::OptionalEffectChoice { .. } => {
                s.runner.act(GameAction::DecideOptionalEffect {
                    accept: !matches!(pick, Pick::Decline),
                })
            }
            WaitingFor::ChooseFromZoneChoice { cards, .. } => {
                s.runner.act(GameAction::SelectCards {
                    cards: chosen(&cards),
                })
            }
            other => panic!("unexpected prompt: {other:?}"),
        };
        result.expect("Brudiclad's trigger accepts the scripted answer");
    }
    targeted
}

fn myr(runner: &GameRunner, exclude: &[ObjectId]) -> ObjectId {
    runner
        .state()
        .battlefield
        .iter()
        .copied()
        .find(|id| runner.state().objects[id].is_token && !exclude.contains(id))
        .expect("the Myr token was created")
}

#[test]
fn every_other_token_becomes_a_copy_of_the_chosen_token() {
    let mut s = setup();
    let goblin = s.goblin;
    let targeted = run_trigger(&mut s, Pick::Existing(goblin));
    let state = s.runner.state();
    let myr = myr(&s.runner, &[s.goblin, s.soldier]);
    assert!(!targeted, "the token is chosen on resolution, not targeted");
    assert_eq!(state.objects[&s.goblin].name, "Goblin");
    assert_eq!(
        state.objects[&s.soldier].name, "Goblin",
        "Soldier becomes a copy"
    );
    assert_eq!(
        state.objects[&myr].name, "Goblin",
        "the new Myr becomes a copy"
    );
    assert_eq!(state.objects[&myr].power, Some(1));
    assert_eq!(
        state.objects[&s.brudiclad].name, "Brudiclad, Telchor Engineer",
        "Brudiclad is not a token and never becomes a copy"
    );
}

#[test]
fn the_new_myr_can_be_the_chosen_token() {
    let mut s = setup();
    let targeted = run_trigger(&mut s, Pick::NewMyr);
    let state = s.runner.state();
    let myr = myr(&s.runner, &[s.goblin, s.soldier]);
    assert!(!targeted, "the token is chosen on resolution, not targeted");
    assert_eq!(state.objects[&myr].name, "Phyrexian Myr");
    assert_eq!(state.objects[&s.goblin].name, "Phyrexian Myr");
    assert_eq!(state.objects[&s.soldier].name, "Phyrexian Myr");
    assert_eq!(state.objects[&s.goblin].power, Some(2));
    assert_eq!(
        state.objects[&s.brudiclad].name,
        "Brudiclad, Telchor Engineer"
    );
}

#[test]
fn declining_the_choice_changes_nothing() {
    let mut s = setup();
    run_trigger(&mut s, Pick::Decline);
    let state = s.runner.state();
    let myr = myr(&s.runner, &[s.goblin, s.soldier]);
    assert_eq!(state.objects[&s.goblin].name, "Goblin");
    assert_eq!(state.objects[&s.soldier].name, "Soldier");
    assert_eq!(state.objects[&myr].name, "Phyrexian Myr");
}
