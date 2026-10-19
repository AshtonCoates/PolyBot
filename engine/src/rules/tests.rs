use super::*;

const JSON: &str = include_str!("../../../re/data/GameLogicData28.json");

fn rules() -> Rules {
    Rules::from_json(JSON).expect("rules load")
}

#[test]
fn table_sizes_match_the_data() {
    let r = rules();
    assert_eq!(r.tribes().len(), 18);
    assert_eq!(r.techs().len(), 49);
    assert_eq!(r.units().len(), 59);
    assert_eq!(r.improvements().len(), 49);
    assert_eq!(r.tasks().len(), 9);
}

#[test]
fn warrior_stats() {
    let r = rules();
    let w = r.unit(r.unit_id("warrior").unwrap());
    assert_eq!((w.health, w.attack, w.defence, w.movement, w.range, w.cost), (100, 20, 20, 1, 1, 2));
    assert!(w.abilities.contains(UnitAbility::Dash));
    assert!(w.abilities.contains(UnitAbility::Fortify));
    assert!(!w.abilities.contains(UnitAbility::Fly));
}

#[test]
fn ids_follow_game_idx_order() {
    let r = rules();
    let idxs: Vec<u32> = r.units().iter().map(|u| u.game_idx).collect();
    assert!(idxs.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(r.unit(UnitId(0)).name, "none");
}

#[test]
fn references_resolve() {
    let r = rules();
    let riding = r.tech(r.tech_id("riding").unwrap());
    assert!(riding.unit_unlocks.contains(&r.unit_id("rider").unwrap()));

    let polaris = r.tribe(r.tribe_id("polaris").unwrap());
    assert!(polaris.unit_overrides.contains(&(r.unit_id("giant").unwrap(), r.unit_id("gaami").unwrap())));

    let dragon = r.unit(r.unit_id("firedragon").unwrap());
    assert_eq!(dragon.upgrades_from, r.unit_id("babydragon"));
}

#[test]
fn multi_key_requirements_keep_both_keys() {
    let r = rules();
    let burn = r.improvement(r.improvement_id("burnforest").unwrap());
    let c = burn.creates[0];
    assert_eq!((c.terrain, c.resource), (Some(Terrain::Field), Some(Resource::Crop)));
}

#[test]
fn resource_terrains() {
    let r = rules();
    let fish = r.resource_terrains(Resource::Fish);
    assert!(fish.contains(Terrain::Water) && fish.contains(Terrain::Ocean));
    assert!(!fish.contains(Terrain::Field));
}

#[test]
fn only_known_dangling_references() {
    // Content referenced by the data but not defined in it; dropped on load.
    let r = rules();
    let mut w = r.warnings.clone();
    w.sort();
    assert_eq!(w.len(), 2, "unexpected warnings: {w:#?}");
    assert!(w.iter().any(|s| s.contains("'whaling'")));
    assert!(w.iter().any(|s| s.contains("'enchantwhale'")));
}
