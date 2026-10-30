//! CR 608.2c + CR 608.2g + CR 118.9 + CR 601.2a + CR 701.20a — Neera, Wild Mage.
//!
//! Accepting "you may cast that card without paying its mana cost" casts the
//! revealed nonland from the library as this ability resolves. The alternative
//! cost is not a later full-price cast from hand. Revealing does not move the
//! card. Cards revealed and not cast go on the bottom of the library.
//!
//! Oracle: "Whenever you cast a spell, you may put it on the bottom of its
//! owner's library. If you do, reveal cards from the top of your library until
//! you reveal a nonland card. You may cast that card without paying its mana
//! cost. Then put all revealed cards not cast this way on the bottom of your
//! library in a random order. This ability triggers only once each turn."

use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::game::triggers::drain_order_triggers_with_identity;
use engine::game::zones::remove_from_zone;
use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{
    AbilityCondition, CardPlayMode, CastFromZoneDriver, Effect, LibraryPosition,
    RevealUntilDisposition, TargetFilter, TypeFilter,
};
use engine::types::actions::{CastChoice, GameAction};
use engine::types::game_state::{CastOfferKind, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const NEERA: &str = "Whenever you cast a spell, you may put it on the bottom of its owner's library. If you do, reveal cards from the top of your library until you reveal a nonland card. You may cast that card without paying its mana cost. Then put all revealed cards not cast this way on the bottom of your library in a random order. This ability triggers only once each turn.";

/// The triggering spell. Its resolution is the only +3 life source.
const GAIN_THREE: &str = "You gain 3 life.";

/// Probe creature. Its enters ability is the only +7 life source, so a life
/// change of +7 proves the spell resolved. The printed cost is unpayable from
/// an empty pool, so a later full-price cast cannot be what moved it.
const ENTERS_GAIN_SEVEN: &str = "When this creature enters, you gain 7 life.";

struct Board {
    runner: GameRunner,
    neera: ObjectId,
    bolt: ObjectId,
    land: ObjectId,
    creature: ObjectId,
    life_before: i32,
}

fn assert_revealed_not_cast_bottom(effect: &Effect) {
    match effect {
        Effect::ChangeZoneAll {
            origin: Some(Zone::Library),
            destination: Zone::Library,
            target: TargetFilter::LastRevealed,
            library_position: Some(LibraryPosition::Bottom),
            random_order: true,
            ..
        } => {}
        other => panic!("expected the uncast revealed cards on the bottom, got {other:?}"),
    }
}

/// CR 701.20b + CR 608.2c + CR 608.2g + CR 118.9: the reveal leaves the cards in
/// the library, the free cast is the singular revealed nonland, and the bottom
/// step is attached on both the accept and decline edges.
#[test]
fn neera_lowers_to_library_reveal_free_cast_and_bottom_cleanup() {
    let parsed = parse_oracle_text(
        NEERA,
        "Neera, Wild Mage",
        &[],
        &["Creature".to_string()],
        &[],
    );
    let execute = parsed.triggers[0]
        .execute
        .as_deref()
        .expect("the spell-cast trigger has an effect");
    assert!(matches!(
        execute.effect.as_ref(),
        Effect::PutAtLibraryPosition {
            target: TargetFilter::TriggeringSource,
            position: LibraryPosition::Bottom,
            ..
        }
    ));
    let reveal = execute
        .sub_ability
        .as_deref()
        .expect("the reveal follows the bottomed spell");
    assert!(reveal
        .condition
        .as_ref()
        .is_some_and(AbilityCondition::is_optional_effect_performed));
    match reveal.effect.as_ref() {
        Effect::RevealUntil {
            matched_disposition,
            kept_destination,
            rest_destination,
            filter,
            ..
        } => {
            assert_eq!(*matched_disposition, RevealUntilDisposition::RevealOnly);
            assert_eq!(*kept_destination, Zone::Library);
            assert_eq!(*rest_destination, Zone::Library);
            let TargetFilter::Typed(typed) = filter else {
                panic!("expected a nonland until-filter, got {filter:?}");
            };
            assert!(typed.type_filters.iter().any(|type_filter| {
                matches!(
                    type_filter,
                    TypeFilter::Non(inner) if matches!(inner.as_ref(), TypeFilter::Land)
                )
            }));
        }
        other => panic!("expected RevealUntil, got {other:?}"),
    }
    let cast = reveal
        .sub_ability
        .as_deref()
        .expect("the free cast follows the reveal");
    assert!(cast.optional);
    assert!(matches!(
        cast.effect.as_ref(),
        Effect::CastFromZone {
            target: TargetFilter::ParentTarget,
            without_paying_mana_cost: true,
            mode: CardPlayMode::Cast,
            driver: CastFromZoneDriver::DuringResolution,
            ..
        }
    ));
    let accept_cleanup = cast
        .sub_ability
        .as_deref()
        .expect("accepted cast still bottoms the uncast revealed cards");
    let decline_cleanup = cast
        .else_ability
        .as_deref()
        .expect("declined cast still bottoms the uncast revealed cards");
    assert_revealed_not_cast_bottom(accept_cleanup.effect.as_ref());
    assert_revealed_not_cast_bottom(decline_cleanup.effect.as_ref());
}

fn board() -> Board {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let neera = scenario
        .add_creature_from_oracle(P0, "Neera, Wild Mage", 2, 7, NEERA)
        .id();
    // Second from the top. The land is placed on top afterwards.
    let creature = scenario
        .add_creature_to_hand_from_oracle(P0, "Revealed Drake", 2, 2, ENTERS_GAIN_SEVEN)
        .with_mana_cost(ManaCost::generic(4))
        .id();
    let land = scenario.add_land_to_library_top(P0, "Island").id();
    let bolt = scenario
        .add_spell_to_hand_from_oracle(P0, "Divergent Bolt", true, GAIN_THREE)
        .id();
    let mut runner = scenario.build();
    {
        let state = runner.state_mut();
        remove_from_zone(state, creature, Zone::Hand, P0);
        state.objects.get_mut(&creature).expect("creature").zone = Zone::Library;
        // Index 0 is the land. The nonland sits under it, so the reveal walks
        // a land and then the creature (CR 701.20a).
        state.library_of_mut(P0).insert(1, creature);
    }
    let life_before = runner
        .state()
        .players
        .iter()
        .find(|player| player.id == P0)
        .expect("player")
        .life;
    Board {
        runner,
        neera,
        bolt,
        land,
        creature,
        life_before,
    }
}

fn cast_bolt(board: &mut Board) {
    let card_id = board.runner.state().objects[&board.bolt].card_id;
    board
        .runner
        .act(GameAction::CastSpell {
            object_id: board.bolt,
            card_id,
            targets: vec![],
            payment_mode: engine::types::game_state::CastPaymentMode::Auto,
        })
        .expect("cast the triggering spell");
}

fn offer_action(kind: &CastOfferKind, accept: bool) -> GameAction {
    let choice = if accept {
        CastChoice::Cast
    } else {
        CastChoice::Decline
    };
    match kind {
        CastOfferKind::Cascade { .. } => GameAction::CascadeChoice { choice },
        CastOfferKind::Discover { .. } => GameAction::DiscoverChoice { choice },
        CastOfferKind::Ripple { .. } => GameAction::RippleChoice { choice },
        CastOfferKind::GraveyardPaidCast { .. } => GameAction::GraveyardPaidCastChoice { choice },
        CastOfferKind::FreeCastWindow { candidates, .. } => GameAction::FreeCastWindowChoice {
            selection: if accept {
                candidates.first().copied()
            } else {
                None
            },
        },
        other => panic!("unhandled cast offer while driving Neera: {other:?}"),
    }
}

/// Accept the bottom-the-spell choice. `accept_free_cast` accepts the next
/// optional or cast offer (the revealed card) and declines any later optional,
/// so a second Neera trigger cannot bottom the spell that was just cast.
fn drive(board: &mut Board, accept_free_cast: bool) -> (Vec<String>, bool) {
    let mut trace = Vec::new();
    let mut optionals_accepted = 0u32;
    let mut cast_offer_answered = false;
    let mut saw_creature_on_stack = false;
    for _ in 0..64 {
        if board.runner.state().objects[&board.creature].zone == Zone::Stack {
            saw_creature_on_stack = true;
        }
        let waiting = board.runner.state().waiting_for.clone();
        trace.push(format!("{waiting:?}"));
        let action = match &waiting {
            WaitingFor::OptionalEffectChoice { .. } => {
                let accept = if optionals_accepted == 0 {
                    true
                } else if accept_free_cast && !cast_offer_answered && optionals_accepted == 1 {
                    true
                } else {
                    false
                };
                if accept {
                    optionals_accepted += 1;
                    if optionals_accepted == 2 {
                        cast_offer_answered = true;
                    }
                }
                GameAction::DecideOptionalEffect { accept }
            }
            WaitingFor::CastOffer { kind, .. } => {
                let action = offer_action(kind, accept_free_cast && !cast_offer_answered);
                cast_offer_answered = true;
                action
            }
            WaitingFor::OrderTriggers { .. } => {
                drain_order_triggers_with_identity(board.runner.state_mut());
                continue;
            }
            WaitingFor::RevealUntilBottomOrder { cards, .. } => GameAction::SelectCards {
                cards: cards.clone(),
            },
            WaitingFor::Priority { .. } => {
                if board.runner.state().stack.is_empty() {
                    break;
                }
                GameAction::PassPriority
            }
            WaitingFor::ManaPayment { .. } | WaitingFor::TargetSelection { .. } => break,
            _ => break,
        };
        if board.runner.act(action).is_err() {
            trace.push("action rejected".to_string());
            break;
        }
    }
    if board.runner.state().objects[&board.creature].zone == Zone::Stack {
        saw_creature_on_stack = true;
    }
    (trace, saw_creature_on_stack)
}

fn zone(board: &Board, id: ObjectId) -> Option<Zone> {
    board
        .runner
        .state()
        .objects
        .get(&id)
        .map(|object| object.zone)
}

fn life(board: &Board) -> i32 {
    board
        .runner
        .state()
        .players
        .iter()
        .find(|player| player.id == P0)
        .expect("player")
        .life
}

fn snapshot(board: &Board, trace: &[String], saw_stack: bool) -> String {
    format!(
        "bolt={:?} land={:?} creature={:?} neera={:?} life {} -> {} saw_stack={saw_stack} waiting={:?}\ntrace:\n{}",
        zone(board, board.bolt),
        zone(board, board.land),
        zone(board, board.creature),
        zone(board, board.neera),
        board.life_before,
        life(board),
        board.runner.state().waiting_for,
        trace.join("\n")
    )
}

/// CR 608.2g + CR 118.9 + CR 601.2a: accepting the free cast puts the revealed
/// nonland on the stack without charging its printed cost, and the spell
/// resolves. The land revealed before it stays out of hand.
#[test]
fn neera_accepting_free_cast_casts_the_revealed_nonland() {
    let mut board = board();
    cast_bolt(&mut board);
    let (trace, saw_stack) = drive(&mut board, true);
    let report = snapshot(&board, &trace, saw_stack);
    assert_eq!(
        zone(&board, board.bolt),
        Some(Zone::Library),
        "the triggering spell is bottomed\n{report}"
    );
    assert_eq!(
        zone(&board, board.neera),
        Some(Zone::Battlefield),
        "Neera stays on the battlefield\n{report}"
    );
    assert!(
        saw_stack,
        "the revealed nonland must be cast onto the stack\n{report}"
    );
    assert_eq!(
        zone(&board, board.creature),
        Some(Zone::Battlefield),
        "the cast creature spell must resolve\n{report}"
    );
    assert_ne!(
        zone(&board, board.creature),
        Some(Zone::Hand),
        "the revealed nonland must not be left in hand at full price\n{report}"
    );
    assert_eq!(
        zone(&board, board.land),
        Some(Zone::Library),
        "the revealed land is not cast and is not put into hand\n{report}"
    );
    assert_eq!(
        life(&board),
        board.life_before + 7,
        "only the revealed creature's enters ability changes life\n{report}"
    );
    assert!(
        matches!(
            board.runner.state().waiting_for,
            WaitingFor::Priority { .. }
        ),
        "resolution must finish at priority\n{report}"
    );
}

/// CR 608.2c + CR 701.20a: declining the free cast does not put the revealed
/// nonland into hand. It joins the other revealed cards on the bottom.
#[test]
fn neera_declining_free_cast_bottoms_the_revealed_nonland() {
    let mut board = board();
    cast_bolt(&mut board);
    let (trace, saw_stack) = drive(&mut board, false);
    let report = snapshot(&board, &trace, saw_stack);
    assert_eq!(
        zone(&board, board.bolt),
        Some(Zone::Library),
        "the triggering spell is bottomed\n{report}"
    );
    assert!(
        !saw_stack,
        "declining must not cast the revealed card\n{report}"
    );
    assert_eq!(
        zone(&board, board.creature),
        Some(Zone::Library),
        "an uncast revealed nonland goes to the bottom of the library\n{report}"
    );
    assert_ne!(
        zone(&board, board.creature),
        Some(Zone::Hand),
        "declining must not deliver the card to hand\n{report}"
    );
    assert_eq!(
        zone(&board, board.land),
        Some(Zone::Library),
        "the revealed land stays in the library\n{report}"
    );
    assert_eq!(
        life(&board),
        board.life_before,
        "neither spell resolves\n{report}"
    );
}
