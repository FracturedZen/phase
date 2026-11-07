//! Personal Decoy (CMB1 playtest card) integration tests.
//!
//! Oracle text:
//! "Personal Decoy enters with a number of loyalty counters on it equal to your life total.
//!  If it would leave the battlefield, exile it instead of putting it anywhere else.
//!  You can't be attacked.
//!  +1: You gain 1 life.
//!  −4: Draw a card."
//!
//! CR 614.1c: ETB loyalty counters equal to life total.
//! CR 614.1 + CR 614.6: Leaves-the-battlefield exile redirect replacement.
//! CR 508.1 + CR 508.1c: "You can't be attacked." prevents declaring attackers against its controller.

use engine::game::combat::{validate_attack_declaration, AttackTarget};
use engine::game::effects::destroy::apply_destroy_after_replacement;
use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::counter::CounterType;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::proposed_event::ProposedEvent;
use engine::types::zones::Zone;

const ORACLE: &str = concat!(
    "Personal Decoy enters with a number of loyalty counters on it equal to your life total. ",
    "If it would leave the battlefield, exile it instead of putting it anywhere else. ",
    "You can't be attacked.\n",
    "+1: You gain 1 life.\n",
    "\u{2212}4: Draw a card."
);

#[test]
fn personal_decoy_etb_counters_cant_be_attacked_and_exile_redirect() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let decoy_id = {
        let mut b = scenario.add_creature_to_hand(P0, "Personal Decoy", 0, 0);
        b.from_oracle_text(ORACLE)
            .as_planeswalker_with_loyalty("Decoy", 0)
            .with_mana_cost(ManaCost::Cost {
                generic: 0,
                shards: vec![],
            });
        b.id()
    };
    let attacker_id = scenario.add_creature(P1, "Grizzly Bears", 2, 2).id();

    let mut runner = scenario.build();
    let _outcome = runner.cast(decoy_id).resolve();
    let state = runner.state_mut();

    // 1. Personal Decoy enters with loyalty counters equal to life total (P0 starts at 20 life).
    // CR 614.1c
    let decoy_obj = state.objects.get(&decoy_id).expect("decoy on battlefield");
    let loyalty_count = decoy_obj
        .counters
        .get(&CounterType::Loyalty)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        loyalty_count, 20,
        "Personal Decoy should enter with 20 loyalty counters"
    );

    // 2. "You can't be attacked." (CR 508.1 / CR 508.1c)
    // The attacker CANNOT attack P0, but CAN attack Personal Decoy (the planeswalker).
    // Set active player to P1 and clear summoning sickness so P1 can declare attacks.
    state.active_player = P1;
    let attacker_obj = state
        .objects
        .get_mut(&attacker_id)
        .expect("attacker exists");
    attacker_obj.summoning_sick = false;

    assert!(
        validate_attack_declaration(state, &[(attacker_id, AttackTarget::Player(P0))], &[])
            .is_err(),
        "P0 cannot be attacked while Personal Decoy is on the battlefield"
    );
    assert!(
        validate_attack_declaration(
            state,
            &[(attacker_id, AttackTarget::Planeswalker(decoy_id))],
            &[]
        )
        .is_ok(),
        "Personal Decoy itself can still be attacked"
    );

    // 3. "If it would leave the battlefield, exile it instead of putting it anywhere else."
    // (CR 614.1 / CR 614.6)
    let destroy_event = ProposedEvent::Destroy {
        object_id: decoy_id,
        source: None,
        cant_regenerate: false,
        applied: Default::default(),
    };
    let mut events = Vec::new();
    let applied = apply_destroy_after_replacement(state, destroy_event, &mut events);
    assert!(applied, "destroy event should resolve");

    let decoy_obj_after = state.objects.get(&decoy_id).expect("decoy still in state");
    assert_eq!(
        decoy_obj_after.zone,
        Zone::Exile,
        "Personal Decoy must be exiled instead of going to graveyard"
    );
}
