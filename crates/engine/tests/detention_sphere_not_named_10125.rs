//! Issue #10125 — Detention Sphere "not named" + same-name mass exile.
//! Standalone integration target so Gate A can run without linking the full
//! `--lib` suite (link OOMs on small boxes).

use engine::parser::oracle::parse_oracle_text;
use engine::types::ability::{Effect, FilterProp, TargetFilter};
use engine::types::zones::Zone;

#[test]
fn detention_sphere_not_named_and_same_name_mass_exile() {
    let parsed = parse_oracle_text(
        "When this enchantment enters, you may exile target nonland permanent not named Detention Sphere and all other permanents with the same name as that permanent.\nWhen this enchantment leaves the battlefield, return the exiled cards to the battlefield under their owner's control.",
        "Detention Sphere",
        &[],
        &["Enchantment".to_string()],
        &[],
    );
    let root = parsed
        .triggers
        .first()
        .and_then(|t| t.execute.as_ref())
        .expect("enters execute");
    assert!(root.optional, "you may exile …");
    let Effect::ChangeZone {
        destination: Zone::Exile,
        target: TargetFilter::Typed(primary),
        ..
    } = &*root.effect
    else {
        panic!("expected ChangeZone(Exile, Typed), got {:?}", root.effect);
    };
    assert!(
        primary.properties.iter().any(|p| matches!(
            p,
            FilterProp::Not { prop }
                if matches!(
                    prop.as_ref(),
                    FilterProp::Named { name }
                        if name.eq_ignore_ascii_case("detention sphere")
                )
        )),
        "primary target must exclude Detention Sphere by name, got {:?}",
        primary.properties
    );
    let sub = root
        .sub_ability
        .as_ref()
        .expect("same-name mass exile continuation");
    let Effect::ChangeZoneAll {
        destination: Zone::Exile,
        target: TargetFilter::Typed(mass),
        ..
    } = &*sub.effect
    else {
        panic!("expected ChangeZoneAll(Exile, Typed), got {:?}", sub.effect);
    };
    assert!(
        mass.properties.contains(&FilterProp::SameNameAsParentTarget),
        "mass exile must restrict to the parent target's name, got {:?}",
        mass.properties
    );
    assert!(
        mass.properties.contains(&FilterProp::Another),
        "mass exile must exclude the targeted permanent itself, got {:?}",
        mass.properties
    );
}

#[test]
fn simple_exile_not_named_keeps_exclusion() {
    let parsed = parse_oracle_text(
        "Exile target nonland permanent not named Detention Sphere.",
        "Probe",
        &[],
        &["Instant".to_string()],
        &[],
    );
    let a = parsed.abilities.first().expect("ability");
    let Effect::ChangeZone {
        destination: Zone::Exile,
        target: TargetFilter::Typed(tf),
        ..
    } = &*a.effect
    else {
        panic!("expected ChangeZone(Exile), got {:?}", a.effect);
    };
    assert!(
        tf.properties.iter().any(|p| matches!(
            p,
            FilterProp::Not { prop }
                if matches!(
                    prop.as_ref(),
                    FilterProp::Named { name }
                        if name.eq_ignore_ascii_case("detention sphere")
                )
        )),
        "Not(Named) missing: {:?}",
        tf.properties
    );
}
