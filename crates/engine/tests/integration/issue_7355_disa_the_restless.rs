//! Issue #7355 — Disa the Restless: "Whenever a Lhurgoyf permanent card is put
//! into your graveyard from anywhere other than the battlefield, put it onto the
//! battlefield." CR 603.6c + CR 603.10a: the trigger must fire for a Lhurgoyf
//! card milled from the library, and must NOT fire for a Lhurgoyf that dies
//! (battlefield → graveyard). Previously the negated-origin clause parsed as
//! `TriggerMode::Unknown`.

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::{CardId, ObjectId};
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::triggers::TriggerMode;
use engine::types::zones::Zone;

// The trailing "{1}: Mill a card." is test scaffolding to drive a
// library → graveyard move; Disa's own printed text is the first line.
const DISA_ORACLE: &str = "Whenever a Lhurgoyf permanent card is put into your \
graveyard from anywhere other than the battlefield, put it onto the battlefield.\n\
{1}: Mill a card.";

fn drain_stack(runner: &mut GameRunner) {
    for _ in 0..200 {
        if matches!(runner.state().waiting_for, WaitingFor::OrderTriggers { .. }) {
            engine::game::triggers::drain_order_triggers_with_identity(runner.state_mut());
            continue;
        }
        match &runner.state().waiting_for {
            WaitingFor::Priority { .. } if runner.state().stack.is_empty() => break,
            _ => {
                if runner.act(GameAction::PassPriority).is_err() {
                    break;
                }
            }
        }
    }
}

fn make_lhurgoyf(runner: &mut GameRunner, zone: Zone) -> ObjectId {
    let state = runner.state_mut();
    let card_id = CardId(state.next_object_id);
    let id = engine::game::zones::create_object(state, card_id, P0, "Goyf".to_string(), zone);
    let obj = state.objects.get_mut(&id).unwrap();
    obj.card_types.core_types.push(CoreType::Creature);
    obj.card_types.subtypes.push("Lhurgoyf".to_string());
    obj.base_card_types = obj.card_types.clone();
    id
}

fn setup() -> (GameRunner, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let disa = scenario
        .add_creature_from_oracle(P0, "Disa the Restless", 5, 6, DISA_ORACLE)
        .id();
    (scenario.build(), disa)
}

#[test]
fn disa_parses_negated_origin_trigger() {
    let (runner, disa) = setup();
    let obj = &runner.state().objects[&disa];
    let entry = obj
        .trigger_definitions
        .iter_unchecked()
        .next()
        .expect("Disa has a trigger");
    assert_eq!(entry.definition.mode, TriggerMode::ChangesZone);
}

#[test]
fn disa_returns_milled_lhurgoyf_to_battlefield() {
    let (mut runner, disa) = setup();
    {
        let p = &mut runner.state_mut().players[0];
        let ids: Vec<_> = p.library.iter().copied().collect();
        p.library.clear();
        for id in ids {
            runner.state_mut().objects.remove(&id);
        }
    }
    let goyf = make_lhurgoyf(&mut runner, Zone::Library);
    runner.state_mut().players[0].mana_pool.add(ManaUnit::new(
        ManaType::Colorless,
        ObjectId(0),
        false,
        vec![],
    ));
    runner
        .act(GameAction::ActivateAbility {
            source_id: disa,
            ability_index: 0,
            payment_mode: CastPaymentMode::Auto,
        })
        .expect("activate mill");
    drain_stack(&mut runner);
    assert_eq!(
        runner.state().objects[&goyf].zone,
        Zone::Battlefield,
        "milled Lhurgoyf must be put onto the battlefield"
    );
}

#[test]
fn disa_does_not_return_lhurgoyf_that_died() {
    let (mut runner, _disa) = setup();
    let goyf = make_lhurgoyf(&mut runner, Zone::Battlefield);
    {
        let obj = runner.state_mut().objects.get_mut(&goyf).unwrap();
        obj.toughness = Some(1);
        obj.power = Some(1);
        obj.damage_marked = 2;
    }
    let mut events = Vec::new();
    engine::game::sba::check_state_based_actions(runner.state_mut(), &mut events);
    engine::game::triggers::process_triggers(runner.state_mut(), &events);
    drain_stack(&mut runner);
    assert_eq!(
        runner.state().objects[&goyf].zone,
        Zone::Graveyard,
        "a Lhurgoyf that dies is not from 'anywhere other than the battlefield'"
    );
}

// ---------------------------------------------------------------------------
// CR 111.11: "create a Tarmogoyf token" — a token created by name only, whose
// name is not a predefined token type, takes the characteristics of the card
// with that name in the Oracle card reference. Ruling (2024-06-07): "Disa the
// Restless's last ability creates a token that's a copy of the card Tarmogoyf
// in the Oracle card reference."
// ---------------------------------------------------------------------------

const DISA_TOKEN_ORACLE: &str = "Whenever one or more creatures you control deal \
combat damage to a player, create a Tarmogoyf token.";

const DOUBLING_SEASON_ORACLE: &str = "If an effect would create one or more tokens \
under your control, it creates twice that many of those tokens instead.\n\
If an effect would put one or more counters on a permanent you control, it puts \
twice that many of those counters on that permanent instead.";

fn graveyard_card(runner: &mut GameRunner, core: CoreType) {
    let state = runner.state_mut();
    let card_id = CardId(state.next_object_id);
    let id = engine::game::zones::create_object(
        state,
        card_id,
        engine::types::player::PlayerId(1),
        format!("{core:?} card"),
        Zone::Graveyard,
    );
    let obj = state.objects.get_mut(&id).unwrap();
    obj.card_types.core_types.push(core);
    obj.base_card_types = obj.card_types.clone();
}

/// Resolve Disa's combat-damage trigger body; returns the created tokens.
fn resolve_disa_token_trigger(with_doubling_season: bool) -> Option<(GameRunner, Vec<ObjectId>)> {
    let db = crate::support::shared_card_db()?;
    let goyf_face = db
        .get_face_by_name("Tarmogoyf")
        .expect("fixture carries Tarmogoyf")
        .clone();
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let disa = scenario
        .add_creature_from_oracle(P0, "Disa the Restless", 5, 6, DISA_TOKEN_ORACLE)
        .id();
    if with_doubling_season {
        scenario.add_enchantment_from_oracle(P0, "Doubling Season", DOUBLING_SEASON_ORACLE);
    }
    let mut runner = scenario.build();
    crate::support::install_synthetic_card_db(runner.state_mut(), &[goyf_face]);
    // Two card types among cards in graveyards: land + instant.
    graveyard_card(&mut runner, CoreType::Land);
    graveyard_card(&mut runner, CoreType::Instant);

    let execute = {
        let obj = &runner.state().objects[&disa];
        let entry = obj
            .trigger_definitions
            .iter_unchecked()
            .next()
            .expect("Disa has the combat-damage trigger");
        entry
            .definition
            .execute
            .as_deref()
            .expect("trigger has an effect")
            .clone()
    };
    let before: Vec<ObjectId> = runner.state().battlefield.iter().copied().collect();
    let resolved = engine::game::ability_utils::build_resolved_from_def(&execute, disa, P0);
    let mut events = Vec::new();
    engine::game::effects::resolve_ability_chain(runner.state_mut(), &resolved, &mut events, 0)
        .expect("Disa's token trigger resolves");
    engine::game::layers::evaluate_layers(runner.state_mut());
    let tokens = runner
        .state()
        .battlefield
        .iter()
        .copied()
        .filter(|id| !before.contains(id))
        .collect();
    Some((runner, tokens))
}

#[test]
fn disa_creates_oracle_tarmogoyf_token() {
    let Some((runner, tokens)) = resolve_disa_token_trigger(false) else {
        return;
    };
    assert_eq!(tokens.len(), 1, "exactly one Tarmogoyf token");
    let token = &runner.state().objects[&tokens[0]];
    assert!(token.is_token);
    assert_eq!(token.name, "Tarmogoyf");
    // CR 111.11 + CR 202.3: the token has Tarmogoyf's mana cost {1}{G}.
    assert_eq!(token.mana_cost.mana_value(), 2);
    assert_eq!(
        token.color,
        vec![engine::types::mana::ManaColor::Green],
        "green"
    );
    assert!(token.card_types.core_types.contains(&CoreType::Creature));
    assert!(token.card_types.subtypes.iter().any(|s| s == "Lhurgoyf"));
    // CR 604.3: Tarmogoyf's CDA — land + instant in graveyards → 2/3.
    assert_eq!(token.power, Some(2), "power = card types in graveyards");
    assert_eq!(token.toughness, Some(3), "toughness = that number plus 1");
}

#[test]
fn disa_tarmogoyf_token_is_doubled_by_doubling_season() {
    let Some((runner, tokens)) = resolve_disa_token_trigger(true) else {
        return;
    };
    // CR 614.1a: Doubling Season doubles the CR 111.11 token creation.
    assert_eq!(tokens.len(), 2, "Doubling Season creates two Tarmogoyfs");
    for id in tokens {
        let token = &runner.state().objects[&id];
        assert!(token.is_token);
        assert_eq!(token.name, "Tarmogoyf");
        assert_eq!(token.mana_cost.mana_value(), 2);
        assert_eq!(token.power, Some(2));
    }
}

fn oracle_token_names(oracle: &str, name: &str) -> Vec<String> {
    let parsed = engine::parser::parse_oracle_text(oracle, name, &[], &[], &[]);
    let json = serde_json::to_string(&parsed).unwrap();
    let mut names = Vec::new();
    let marker = "\"type\":\"CreateTokenFromOracleCard\"";
    for (i, _) in json.match_indices(marker) {
        let tail = &json[i..];
        if let Some(start) = tail.find("\"name\":\"") {
            let rest = &tail[start + 8..];
            names.push(rest[..rest.find('"').unwrap()].to_string());
        }
    }
    names
}

/// CR 111.11 + MB2 playtest rulings (Dwarven Confluencer, Ginger, Disa): a token
/// named after a real card is a copy of that Oracle card; a "(They're …)" /
/// "(It's a land with …)" parenthetical only summarizes it.
#[test]
fn worzel_scryb_sprites_reminder_is_summary_not_definition() {
    let oracle = "\u{2212}8: Create ten Scryb Sprites tokens. (They're {G} 1/1 Faerie creatures with flying.)";
    assert_eq!(
        oracle_token_names(oracle, "Worzel, the Protector"),
        vec!["Scryb Sprites".to_string()]
    );
}

#[test]
fn waste_land_creates_oracle_wastes_token() {
    let oracle = "{T}, Sacrifice Waste Land: Destroy target nonbasic land. That land's controller creates a Wastes token. (It's a land with {T}: Add {C}.)";
    assert_eq!(oracle_token_names(oracle, "Waste Land"), vec!["Wastes".to_string()]);
}

/// CR 111.10/111.4: Scythe is not a card name; the reminder defines it, so no lookup.
#[test]
fn keeper_scythe_reminder_defined_token_is_not_oracle_lookup() {
    let oracle = "When The Keeper of Four Scythes enters, create four Scythe tokens. (They're Equipment with \"Equipped creature gets +1/+1\" and equip {2}.)";
    assert!(oracle_token_names(oracle, "The Keeper of Four Scythes").is_empty());
}
