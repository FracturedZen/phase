//! Issue #8129 — Kavaron, Memorial World (Land — Planet).
//!
//! CR 702.184a + CR 721.2: a Planet's `N+ | body` striation is gated on its
//! charge counters exactly like a Spacecraft's. The threshold pre-parser only
//! ran for the Spacecraft subtype, so a Planet's `12+ | {cost}: effect` line
//! fell through to the generic activated-ability parser with no
//! `CounterThreshold` restriction — usable at 0 or 4 charge counters.

use engine::ai_support::legal_actions;
use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::ActivationRestriction;
use engine::types::actions::GameAction;
use engine::types::counter::{CounterMatch, CounterType};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;

const KAVARON: &str = "This land enters tapped.\n{T}: Add {R}.\nStation (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n12+ | {1}{R}, {T}, Sacrifice a land: Create a 2/2 colorless Robot artifact creature token, then creatures you control get +1/+0 and gain haste until end of turn.";

const EVENDO: &str = "This land enters tapped.\n{T}: Add {G}.\nStation (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n12+ | {G}, {T}: Add {G} for each creature you control.";

fn has_charge_gate(restrictions: &[ActivationRestriction], n: u32) -> bool {
    restrictions.iter().any(|r| {
        matches!(
            r,
            ActivationRestriction::CounterThreshold {
                counters: CounterMatch::OfType(CounterType::Generic(c)),
                minimum,
                maximum: None,
            } if c == "charge" && *minimum == n
        )
    })
}

fn parse_planet(text: &str, name: &str) -> engine::parser::oracle::ParsedAbilities {
    parse_oracle_text(
        text,
        name,
        &["Station".to_string()],
        &["Land".to_string()],
        &["Planet".to_string()],
    )
}

/// CR 721.2: both the non-mana (Kavaron) and mana (Evendo) striation bodies on
/// a Planet carry the `12+` charge-counter activation gate.
#[test]
fn planet_threshold_activated_abilities_carry_charge_gate() {
    for (name, text) in [
        ("Kavaron, Memorial World", KAVARON),
        ("Evendo, Waking Haven", EVENDO),
    ] {
        let parsed = parse_planet(text, name);
        let gated: Vec<_> = parsed
            .abilities
            .iter()
            .filter(|a| {
                a.description
                    .as_deref()
                    .is_some_and(|d| d.contains("for each") || d.contains("Robot"))
            })
            .collect();
        assert_eq!(gated.len(), 1, "{name}: expected one threshold ability");
        assert!(
            has_charge_gate(&gated[0].activation_restrictions, 12),
            "{name}: 12+ ability must carry CounterThreshold(charge, 12), got {:?}",
            gated[0].activation_restrictions
        );
        // The base `{T}: Add {X}.` ability stays ungated.
        let base = parsed
            .abilities
            .iter()
            .find(|a| {
                a.description
                    .as_deref()
                    .is_some_and(|d| d.starts_with("{T}: Add"))
            })
            .expect("base mana ability");
        assert!(base.activation_restrictions.is_empty());
    }
}

fn kavaron_runner() -> (GameRunner, ObjectId, usize) {
    let mut s = GameScenario::new_n_player(2, 42);
    s.at_phase(Phase::PreCombatMain);
    let k = s
        .add_land_from_oracle(P0, "Kavaron, Memorial World", "")
        .with_subtypes(vec!["Planet"])
        .from_oracle_text_with_keywords(&["Station"], KAVARON)
        .id();
    s.add_creature(P0, "Bear", 3, 3);
    s.add_basic_land(P0, engine::types::mana::ManaColor::Red);
    s.with_mana_pool(
        P0,
        vec![
            ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![]),
        ],
    );
    let r = s.build();
    let idx = r.state().objects[&k]
        .abilities
        .iter()
        .position(|a| {
            a.description
                .as_deref()
                .is_some_and(|d| d.contains("Robot"))
        })
        .expect("Kavaron robot ability");
    (r, k, idx)
}

fn set_charge(r: &mut GameRunner, id: ObjectId, n: u32) {
    r.state_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .counters
        .insert(CounterType::Generic("charge".to_string()), n);
}

fn robot_offered(r: &GameRunner, k: ObjectId, idx: usize) -> bool {
    legal_actions(r.state()).iter().any(|a| {
        matches!(a, GameAction::ActivateAbility { source_id, ability_index, .. }
            if *source_id == k && *ability_index == idx)
    })
}

/// CR 721.2: below 12 charge counters the Robot ability is neither offered nor
/// activatable; at 12 it is. The Station action itself stays available.
#[test]
fn kavaron_robot_ability_requires_twelve_charge_counters() {
    let (mut r, k, idx) = kavaron_runner();

    for n in [0, 4, 11] {
        set_charge(&mut r, k, n);
        assert!(
            !robot_offered(&r, k, idx),
            "Robot ability must not be offered at {n} charge counters"
        );
        assert!(
            r.act(GameAction::ActivateAbility {
                source_id: k,
                ability_index: idx,
                payment_mode: Default::default(),
            })
            .is_err(),
            "Robot ability must be rejected at {n} charge counters"
        );
    }
    assert!(
        legal_actions(r.state())
            .iter()
            .any(|a| matches!(a, GameAction::ActivateStation { spacecraft_id, .. } if *spacecraft_id == k)),
        "Station must be offered on the Planet"
    );

    set_charge(&mut r, k, 12);
    assert!(
        robot_offered(&r, k, idx),
        "Robot ability must be offered at 12 charge counters"
    );
}
