//! #10400: Anger's graveyard static must apply as soon as Anger is milled.
//! CR 113.6b + CR 611.3a: "As long as this card is in your graveyard and you
//! control a Mountain, creatures you control have haste" functions from the
//! graveyard, so it starts applying when the card arrives there and stops
//! when it leaves, with no unrelated layer invalidation needed.

use super::rules::{GameRunner, GameScenario, Keyword, ObjectId, Phase, Zone, P0};
use engine::types::mana::{ManaColor, ManaCost};
use engine::types::statics::StaticMode;

const ANGER: &str = "Haste\nWhen Anger dies, return it to its owner's hand.\nAs long as Anger is in your graveyard and you control a Mountain, creatures you control have haste.";
const MILL: &str = "Mill a card.";
const EXILE_FROM_GRAVEYARD: &str = "Exile target card from a graveyard.";

struct Setup {
    runner: GameRunner,
    anger: ObjectId,
    bear: ObjectId,
    mill: ObjectId,
    exile: ObjectId,
}

fn setup(with_mountain: bool) -> Setup {
    let mut s = GameScenario::new();
    s.at_phase(Phase::PreCombatMain);
    if with_mountain {
        s.add_basic_land(P0, ManaColor::Red);
    }
    let bear = s.add_creature(P0, "Bear", 2, 2).id();
    let anger = s
        .add_creature_to_library_top(P0, "Anger", 2, 2)
        .from_oracle_text(ANGER)
        .id();
    let mill = s
        .add_spell_to_hand_from_oracle(P0, "Tome Probe", false, MILL)
        .with_mana_cost(ManaCost::zero())
        .id();
    let exile = s
        .add_spell_to_hand_from_oracle(P0, "Grave Probe", false, EXILE_FROM_GRAVEYARD)
        .with_mana_cost(ManaCost::zero())
        .id();
    let runner = s.build();
    // Fixture guard: Anger's Oracle text parsed to a graveyard-functioning static.
    assert!(
        runner.state().objects[&anger]
            .static_definitions
            .iter_unchecked()
            .any(|d| d.mode == StaticMode::Continuous && d.active_zones.contains(&Zone::Graveyard)),
        "Anger must carry its graveyard static"
    );
    Setup {
        runner,
        anger,
        bear,
        mill,
        exile,
    }
}

fn bear_has_haste(runner: &GameRunner, bear: ObjectId) -> bool {
    runner.state().objects[&bear].has_keyword(&Keyword::Haste)
}

#[test]
fn milled_anger_grants_haste_immediately() {
    let Setup {
        mut runner,
        anger,
        bear,
        mill,
        ..
    } = setup(true);
    assert!(
        !bear_has_haste(&runner, bear),
        "no haste while Anger is in the library"
    );

    runner.cast(mill).resolve();

    assert_eq!(runner.state().objects[&anger].zone, Zone::Graveyard);
    assert!(
        bear_has_haste(&runner, bear),
        "CR 113.6b + CR 611.3a: milled Anger's graveyard static grants haste at once"
    );
}

#[test]
fn anger_leaving_graveyard_removes_haste() {
    let Setup {
        mut runner,
        anger,
        bear,
        mill,
        exile,
    } = setup(true);
    runner.cast(mill).resolve();
    assert!(bear_has_haste(&runner, bear));

    runner.cast(exile).target_object(anger).resolve();

    assert_eq!(runner.state().objects[&anger].zone, Zone::Exile);
    assert!(
        !bear_has_haste(&runner, bear),
        "CR 611.3a: once Anger leaves the graveyard its static stops applying"
    );
}

#[test]
fn milled_anger_without_mountain_grants_nothing() {
    let Setup {
        mut runner,
        anger,
        bear,
        mill,
        ..
    } = setup(false);
    runner.cast(mill).resolve();

    assert_eq!(runner.state().objects[&anger].zone, Zone::Graveyard);
    assert!(
        !bear_has_haste(&runner, bear),
        "Anger's condition also requires a Mountain"
    );
}
