//! Issue #8125 — Reinterpret: "Counter target spell. You may cast a spell with
//! equal or lesser mana value from your hand without paying its mana cost."
//!
//! When casting Reinterpret, the second clause ("You may cast a spell with equal
//! or lesser mana value from your hand without paying its mana cost") does not
//! target — the spell is chosen at resolution from the caster's hand (CR 115.1,
//! CR 601.2c, CR 608.2d).
//!
//! Previously, the CastFromZone sub-ability inherited the parent Counter
//! ability's object target (the countered spell), causing the resolver to skip
//! opening a resolution-time hand pick and instead attempt to cast the countered
//! spell (e.g. the opponent's commander) from the graveyard.
//!
//! Under CR 608.2d and CR 608.2h:
//! - The target spell on the stack is countered and sent to its owner's graveyard
//!   (or command zone/etc.).
//! - The caster is offered the optional "may" choice.
//! - On accept, a resolution-time hand selection (`WaitingFor::EffectZoneChoice`)
//!   opens for the caster's hand, offering only spells with mana value less than
//!   or equal to the countered spell's mana value (derived from its last known
//!   information).
//! - The chosen spell from hand is cast without paying its mana cost.

use engine::game::scenario::{CastCommit, GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::{CastingVariant, StackEntry, StackEntryKind, WaitingFor};
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const REINTERPRET_ORACLE: &str = "Counter target spell. You may cast a spell with equal or \
                                  lesser mana value from your hand without paying its mana cost.";

struct Board {
    runner: GameRunner,
    reinterpret: ObjectId,
    p0_small_spell: ObjectId,
    p0_big_spell: ObjectId,
    p1_hand_spell: ObjectId,
}

fn setup() -> Board {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let reinterpret = scenario
        .add_spell_to_hand_from_oracle(P0, "Reinterpret", true, REINTERPRET_ORACLE)
        .with_mana_cost(ManaCost::default())
        .id();

    let p0_small_spell = scenario
        .add_spell_to_hand(P0, "P0 Small Spell", true)
        .with_mana_cost(ManaCost::generic(2))
        .id();
    let p0_big_spell = scenario
        .add_spell_to_hand(P0, "P0 Big Spell", true)
        .with_mana_cost(ManaCost::generic(5))
        .id();
    let p1_hand_spell = scenario
        .add_spell_to_hand(P1, "P1 Hand Spell", true)
        .with_mana_cost(ManaCost::generic(1))
        .id();

    Board {
        runner: scenario.build(),
        reinterpret,
        p0_small_spell,
        p0_big_spell,
        p1_hand_spell,
    }
}

fn put_spell_on_stack(
    runner: &mut GameRunner,
    controller: PlayerId,
    name: &str,
    mana_value: u32,
) -> ObjectId {
    let spell = engine::game::zones::create_object(
        runner.state_mut(),
        CardId(900),
        controller,
        name.to_string(),
        Zone::Stack,
    );
    runner
        .state_mut()
        .objects
        .get_mut(&spell)
        .unwrap()
        .card_types
        .core_types = vec![CoreType::Instant];
    runner
        .state_mut()
        .objects
        .get_mut(&spell)
        .unwrap()
        .mana_cost = ManaCost::generic(mana_value);
    runner.state_mut().stack.push_back(StackEntry {
        id: spell,
        source_id: spell,
        controller,
        kind: StackEntryKind::Spell {
            card_id: CardId(900),
            ability: None,
            casting_variant: CastingVariant::Normal,
            actual_mana_spent: mana_value,
        },
    });
    spell
}

fn pass_until_reinterpret_prompts(commit: &mut CastCommit<'_>, reinterpret: ObjectId) {
    for _ in 0..6 {
        let on_stack = commit.state().objects[&reinterpret].zone == Zone::Stack;
        match commit.state().waiting_for {
            WaitingFor::Priority { .. } if on_stack => {
                commit
                    .act(GameAction::PassPriority)
                    .expect("passing priority must be accepted");
            }
            _ => return,
        }
    }
    panic!(
        "Reinterpret never prompted, waiting_for: {:?}",
        commit.state().waiting_for
    );
}

#[test]
fn reinterpret_casts_eligible_spell_from_caster_hand_not_countered_spell() {
    let Board {
        mut runner,
        reinterpret,
        p0_small_spell,
        p0_big_spell,
        p1_hand_spell,
    } = setup();

    // P1 has a 3-cost spell on the stack.
    let opponent_spell = put_spell_on_stack(&mut runner, P1, "Opponent Commander", 3);

    // P0 casts Reinterpret targeting the opponent's spell.
    let mut commit = runner
        .cast(reinterpret)
        .target_objects(&[opponent_spell])
        .commit();
    pass_until_reinterpret_prompts(&mut commit, reinterpret);

    // CR 608.2d: P0 is offered the optional "may cast" choice.
    let WaitingFor::OptionalEffectChoice { player, .. } = commit.state().waiting_for else {
        panic!(
            "P0 must be offered the optional cast, got {:?}",
            commit.state().waiting_for
        );
    };
    assert_eq!(player, P0, "P0 decides the optional cast");

    commit
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accepting optional cast must succeed");

    // CR 608.2d + CR 608.2h: The pick must be from P0's hand, offering cards
    // with MV <= 3 (the countered spell's MV). It must NOT offer the countered
    // spell itself, nor P0's 5-MV card, nor P1's hand card.
    let WaitingFor::EffectZoneChoice {
        player,
        cards,
        zone,
        ..
    } = &commit.state().waiting_for
    else {
        panic!(
            "accepting must open an EffectZoneChoice for hand, got {:?}",
            commit.state().waiting_for
        );
    };

    assert_eq!(*player, P0, "P0 is choosing from hand");
    assert_eq!(*zone, Zone::Hand, "choice is from hand");
    assert!(
        cards.contains(&p0_small_spell),
        "P0's 2-MV spell is eligible (<= 3 MV)"
    );
    assert!(
        !cards.contains(&p0_big_spell),
        "P0's 5-MV spell is not eligible (> 3 MV)"
    );
    assert!(
        !cards.contains(&opponent_spell),
        "the countered spell is NOT a candidate for hand cast"
    );
    assert!(
        !cards.contains(&p1_hand_spell),
        "opponent's hand is not candidate"
    );

    // P0 selects the small spell.
    commit
        .act(GameAction::SelectCards {
            cards: vec![p0_small_spell],
        })
        .expect("selecting eligible card from hand must succeed");

    // The chosen card should now be cast and on the stack under P0's control.
    let state = commit.state();
    assert_eq!(
        state.objects[&p0_small_spell].zone,
        Zone::Stack,
        "chosen card is cast onto the stack (CR 608.2g)"
    );
    assert_eq!(
        state.objects[&p0_small_spell].controller, P0,
        "chosen card is controlled by P0"
    );
    assert_eq!(
        state.objects[&opponent_spell].zone,
        Zone::Graveyard,
        "countered spell is in the graveyard (CR 701.6a)"
    );
    assert_eq!(
        state.objects[&reinterpret].zone,
        Zone::Graveyard,
        "Reinterpret is in the graveyard after resolution"
    );
}

#[test]
fn reinterpret_declining_optional_casts_nothing() {
    let Board {
        mut runner,
        reinterpret,
        p0_small_spell,
        p0_big_spell,
        ..
    } = setup();

    let opponent_spell = put_spell_on_stack(&mut runner, P1, "Opponent Commander", 3);

    let mut commit = runner
        .cast(reinterpret)
        .target_objects(&[opponent_spell])
        .commit();
    pass_until_reinterpret_prompts(&mut commit, reinterpret);

    commit
        .act(GameAction::DecideOptionalEffect { accept: false })
        .expect("declining optional cast must succeed");

    let state = commit.state();
    assert_eq!(
        state.objects[&opponent_spell].zone,
        Zone::Graveyard,
        "opponent spell was countered"
    );
    assert_eq!(
        state.objects[&p0_small_spell].zone,
        Zone::Hand,
        "small spell remains in hand"
    );
    assert_eq!(
        state.objects[&p0_big_spell].zone,
        Zone::Hand,
        "big spell remains in hand"
    );
    assert_eq!(
        state.objects[&reinterpret].zone,
        Zone::Graveyard,
        "Reinterpret resolved and is in graveyard"
    );
}
