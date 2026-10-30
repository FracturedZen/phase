//! Issue #8139: Bilbo, Thief in the Night — "Whenever Bilbo attacks, you may
//! cast an artifact, instant, or sorcery spell from your graveyard. If an
//! instant or sorcery spell cast this way would be put into your graveyard,
//! exile it instead."
//!
//! The instruction names no card (no "target"), so the card is chosen while the
//! trigger resolves and cast during that resolution, paying its cost
//! (CR 608.2g, CR 601.2a-h). The free form of this shape (Diviner of Mist)
//! already opened that pick; the PAID form fell through to "no targets
//! resolved", so accepting the "you may" did nothing.

use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaColor, ManaCost, ManaCostShard};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

use super::cast_this_way_gate_8721::to_declare_attackers;
use super::rules::AttackTarget;

const BILBO: &str = "Spells you cast from anywhere other than your hand cost {1} less to cast.\n\
Whenever Bilbo attacks, you may cast an artifact, instant, or sorcery spell from your graveyard. \
If an instant or sorcery spell cast this way would be put into your graveyard, exile it instead.";

fn tapped_lands(runner: &GameRunner) -> usize {
    runner
        .state()
        .objects
        .values()
        .filter(|o| {
            o.controller == P0 && o.tapped && o.card_types.core_types.contains(&CoreType::Land)
        })
        .count()
}

/// Attack with Bilbo and accept the "you may", stopping at the first
/// non-trigger-driving prompt.
fn attack_and_accept(runner: &mut GameRunner, bilbo: ObjectId) {
    to_declare_attackers(runner, P0);
    runner
        .declare_attackers(&[(bilbo, AttackTarget::Player(P1))])
        .expect("Bilbo must be a legal attacker");
    for _ in 0..20 {
        match runner.state().waiting_for.clone() {
            WaitingFor::OptionalEffectChoice { .. } => {
                runner
                    .act(GameAction::DecideOptionalEffect { accept: true })
                    .expect("accept Bilbo's may");
            }
            WaitingFor::Priority { .. } if !runner.state().stack.is_empty() => {
                runner.act(GameAction::PassPriority).expect("pass");
            }
            _ => return,
        }
    }
}

fn graveyard_pool(runner: &GameRunner) -> Vec<ObjectId> {
    match &runner.state().waiting_for {
        WaitingFor::EffectZoneChoice {
            cards,
            zone: Zone::Graveyard,
            count: 1,
            up_to: true,
            ..
        } => {
            let mut cards = cards.clone();
            cards.sort();
            cards
        }
        other => panic!("Bilbo's trigger offers a graveyard pick, found {other:?}"),
    }
}

/// Pay for the selected spell (choosing any targets it needs).
fn pay(runner: &mut GameRunner) {
    for _ in 0..12 {
        match runner.state().waiting_for {
            WaitingFor::ManaPayment { .. } => {
                runner.act(GameAction::PassPriority).expect("pay from lands");
            }
            WaitingFor::TargetSelection { .. } => {
                runner.choose_first_legal_target().expect("choose a target");
            }
            _ => return,
        }
    }
}

struct Board {
    runner: GameRunner,
    bilbo: ObjectId,
    bolt: ObjectId,
    sorcery: ObjectId,
    creature: ObjectId,
}

fn board() -> Board {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let bilbo = scenario
        .add_creature_from_oracle(P0, "Bilbo, Thief in the Night", 1, 2, BILBO)
        .id();
    for _ in 0..3 {
        scenario.add_basic_land(P0, ManaColor::Red);
    }
    let bolt = scenario
        .add_spell_to_graveyard(P0, "Lightning Bolt", true)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Red],
            generic: 1,
        })
        .id();
    let sorcery = scenario
        .add_spell_to_graveyard(P0, "Some Sorcery", false)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Red],
            generic: 2,
        })
        .id();
    let creature = scenario.add_creature_to_graveyard(P0, "Grizzly Bears", 2, 2).id();
    let runner = scenario.build();
    Board {
        runner,
        bilbo,
        bolt,
        sorcery,
        creature,
    }
}

/// CR 608.2g + CR 601.2f: accepting opens a pick of the castable artifact,
/// instant and sorcery cards in Bilbo's controller's graveyard (not the
/// creature card); the chosen instant is cast during the trigger's
/// resolution, paying its cost less Bilbo's {1} reduction, and is exiled
/// instead of going to the graveyard when it resolves.
#[test]
fn bilbo_attack_casts_a_paid_instant_from_the_graveyard() {
    let Board {
        mut runner,
        bilbo,
        bolt,
        sorcery,
        creature,
    } = board();
    attack_and_accept(&mut runner, bilbo);

    let mut expected = vec![bolt, sorcery];
    expected.sort();
    assert_eq!(
        graveyard_pool(&runner),
        expected,
        "instant and sorcery are offered; the creature card {creature:?} is not"
    );
    assert_eq!(tapped_lands(&runner), 0, "reach guard: nothing paid yet");

    runner
        .act(GameAction::SelectCards { cards: vec![bolt] })
        .expect("pick the instant");
    pay(&mut runner);
    assert_eq!(
        runner.state().objects[&bolt].zone,
        Zone::Stack,
        "the instant is cast while the trigger resolves, found {:?}",
        runner.state().waiting_for
    );
    assert_eq!(
        tapped_lands(&runner),
        1,
        "{{1}}{{R}} less Bilbo's {{1}} reduction (cast from outside the hand) is {{R}}"
    );
    assert_eq!(runner.state().phase, Phase::DeclareAttackers);

    runner.advance_until_stack_empty();
    assert_eq!(
        runner.state().objects[&bolt].zone,
        Zone::Exile,
        "the rider exiles the instant instead of putting it into the graveyard"
    );
}

/// CR 608.2g: card-type timing permissions are ignored — a sorcery chosen from
/// the graveyard is cast in the declare attackers step.
#[test]
fn bilbo_attack_casts_a_sorcery_in_combat() {
    let Board {
        mut runner,
        bilbo,
        sorcery,
        ..
    } = board();
    attack_and_accept(&mut runner, bilbo);
    runner
        .act(GameAction::SelectCards {
            cards: vec![sorcery],
        })
        .expect("pick the sorcery");
    pay(&mut runner);
    assert_eq!(
        runner.state().objects[&sorcery].zone,
        Zone::Stack,
        "the sorcery is cast in combat, found {:?}",
        runner.state().waiting_for
    );
    assert_eq!(runner.state().phase, Phase::DeclareAttackers);
    assert_eq!(
        tapped_lands(&runner),
        2,
        "{{2}}{{R}} less Bilbo's {{1}} reduction"
    );
}

/// CR 608.2d: choosing no card casts nothing; the cards stay in the graveyard.
#[test]
fn bilbo_pick_of_nothing_casts_nothing() {
    let Board {
        mut runner,
        bilbo,
        bolt,
        sorcery,
        ..
    } = board();
    attack_and_accept(&mut runner, bilbo);
    let _ = graveyard_pool(&runner);
    runner
        .act(GameAction::SelectCards { cards: vec![] })
        .expect("choose none");
    runner.advance_until_stack_empty();
    assert_eq!(runner.state().objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(runner.state().objects[&sorcery].zone, Zone::Graveyard);
    assert_eq!(tapped_lands(&runner), 0);
}
