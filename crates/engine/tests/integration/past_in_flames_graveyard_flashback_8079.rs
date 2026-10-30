//! Issue #8079 — Past in Flames: "Each instant and sorcery card in your
//! graveyard gains flashback until end of turn. The flashback cost is equal to
//! its mana cost."
//!
//! CR 702.34a: flashback lets a card be cast from its owner's graveyard.
//! CR 611.2c: a continuous effect from a resolving spell that modifies
//! characteristics affects only the objects determined when it began — the
//! instant and sorcery cards in your graveyard at resolution, not cards put
//! there later. The broadcast grant previously scanned only the battlefield,
//! so no graveyard card ever gained flashback.

use std::collections::HashSet;

use engine::game::casting::spell_objects_available_to_cast;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::ability::TargetFilter;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const PAST_IN_FLAMES: &str = "Each instant and sorcery card in your graveyard gains flashback until end of turn. The flashback cost is equal to its mana cost.\nFlashback {4}{R} (You may cast this card from your graveyard for its flashback cost. Then exile it.)";

fn bound_recipients(state: &engine::types::game_state::GameState) -> HashSet<ObjectId> {
    state
        .transient_continuous_effects
        .iter()
        .filter_map(|e| match &e.affected {
            TargetFilter::SpecificObject { id } => Some(*id),
            _ => None,
        })
        .collect()
}

#[test]
fn past_in_flames_grants_flashback_to_own_graveyard_instants_and_sorceries() {
    let mut sc = GameScenario::new();
    sc.at_phase(Phase::PreCombatMain);
    sc.with_library_top(P0, &["Island", "Island", "Island", "Island", "Island"]);
    let divination = sc
        .add_spell_to_graveyard(P0, "Divination", false)
        .from_oracle_text("Draw two cards.")
        .id();
    let opt = sc
        .add_spell_to_graveyard(P0, "Opt", true)
        .from_oracle_text("Scry 1.\nDraw a card.")
        .id();
    let creature = sc.add_creature_to_graveyard(P0, "Grizzly Bears", 2, 2).id();
    let opp_instant = sc
        .add_spell_to_graveyard(P1, "Brainstorm", true)
        .from_oracle_text("Draw three cards, then put two cards from your hand on top of your library in any order.")
        .id();
    let pif = sc
        .add_spell_to_hand_from_oracle(P0, "Past in Flames", false, PAST_IN_FLAMES)
        .id();
    let mut runner = sc.build();

    assert!(
        !spell_objects_available_to_cast(runner.state(), P0).contains(&divination),
        "precondition: Divination is not castable from the graveyard before Past in Flames"
    );

    runner.cast(pif).resolve();

    let bound = bound_recipients(runner.state());
    assert!(bound.contains(&divination), "Divination should gain flashback");
    assert!(bound.contains(&opt), "Opt should gain flashback");
    assert!(
        !bound.contains(&creature),
        "a creature card in the graveyard must not gain flashback"
    );
    assert!(
        !bound.contains(&opp_instant),
        "an opponent's graveyard instant must not gain flashback (\"your graveyard\")"
    );
    assert!(
        !bound.contains(&pif),
        "Past in Flames itself is on the stack at resolution, not in the graveyard"
    );

    let castable = spell_objects_available_to_cast(runner.state(), P0);
    assert!(castable.contains(&divination));
    assert!(castable.contains(&opt));
    assert!(
        !spell_objects_available_to_cast(runner.state(), P1).contains(&opp_instant),
        "the opponent gains no flashback from Past in Flames"
    );

    // CR 702.34a: cast with flashback, then exiled instead of going anywhere else.
    runner
        .cast(divination)
        .try_resolve()
        .expect("Divination should be castable via granted flashback");
    assert_eq!(runner.state().objects[&divination].zone, Zone::Exile);
    assert!(
        !matches!(
            runner.state().waiting_for,
            engine::types::game_state::WaitingFor::GameOver { .. }
        ),
        "the game should continue after the flashback cast"
    );
}

#[test]
fn past_in_flames_does_not_affect_cards_put_into_graveyard_later() {
    // CR 611.2c: the affected set is locked when Past in Flames resolves.
    let mut sc = GameScenario::new();
    sc.at_phase(Phase::PreCombatMain);
    sc.with_library_top(P0, &["Island", "Island", "Island", "Island", "Island"]);
    let pif = sc
        .add_spell_to_hand_from_oracle(P0, "Past in Flames", false, PAST_IN_FLAMES)
        .id();
    let later = sc
        .add_spell_to_hand_from_oracle(P0, "Divination", false, "Draw two cards.")
        .id();
    let mut runner = sc.build();

    runner.cast(pif).resolve();
    runner.cast(later).resolve();
    assert_eq!(runner.state().objects[&later].zone, Zone::Graveyard);
    assert!(
        !spell_objects_available_to_cast(runner.state(), P0).contains(&later),
        "a card put into the graveyard after Past in Flames resolved has no flashback"
    );
}
