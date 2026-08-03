//! Exit criterion 3: property tests for the D-010 scope-overlap function.
//!
//! The generator is a hand-rolled deterministic LCG rather than a proptest dependency:
//! the domain is a handful of term shapes, and `docs/13` §4 asks for the dependency list
//! to stay short.

use akr_core::model::{Glob, LogicalKey, PartOfIndex, Reference, ScopeTerm, scopes_overlap};

/// A deterministic linear congruential generator, seeded per test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

const PATHS: &[&str] = &[
    "sim/**",
    "sim/src/project/**",
    "sim/src/step.rs",
    "sim/*/mod.rs",
    "sim/tests/**",
    "lege/**",
    "lege/src/render/**",
    "content/**/light/**",
    "docs/generated/**",
];

const KEYS: &[&str] = &[
    "sys.track.lighting",
    "sys.track.perf-watch",
    "sys.milestone.m3-playable-day",
    "sys.constraint.frame-budget-16ms",
];

fn term(rng: &mut Rng) -> ScopeTerm {
    match rng.below(10) {
        0 => ScopeTerm::All,
        1..=4 => ScopeTerm::Ref(Reference::head(
            LogicalKey::parse(KEYS[rng.below(KEYS.len())]).expect("valid key"),
        )),
        _ => ScopeTerm::Path(Glob::new(PATHS[rng.below(PATHS.len())])),
    }
}

fn scope(rng: &mut Rng) -> Vec<ScopeTerm> {
    let n = 1 + rng.below(3);
    (0..n).map(|_| term(rng)).collect()
}

fn index() -> PartOfIndex {
    let mut index = PartOfIndex::empty();
    index.insert(
        LogicalKey::parse("sys.milestone.m3-playable-day").expect("valid key"),
        LogicalKey::parse("sys.track.lighting").expect("valid key"),
    );
    index
}

#[test]
fn overlap_is_reflexive_for_any_non_empty_scope() {
    let mut rng = Rng(0x5eed_0001);
    let parents = index();
    for _ in 0..2_000 {
        let s = scope(&mut rng);
        assert!(
            scopes_overlap(&s, &s, &parents),
            "{s:?} does not overlap itself"
        );
    }
}

#[test]
fn overlap_is_symmetric() {
    let mut rng = Rng(0x5eed_0002);
    let parents = index();
    for _ in 0..2_000 {
        let (a, b) = (scope(&mut rng), scope(&mut rng));
        assert_eq!(
            scopes_overlap(&a, &b, &parents),
            scopes_overlap(&b, &a, &parents),
            "asymmetric for {a:?} and {b:?}"
        );
    }
}

#[test]
fn all_overlaps_everything() {
    let mut rng = Rng(0x5eed_0003);
    let parents = index();
    let everything = vec![ScopeTerm::All];
    for _ in 0..2_000 {
        let s = scope(&mut rng);
        assert!(
            scopes_overlap(&everything, &s, &parents),
            "`all` missed {s:?}"
        );
        assert!(
            scopes_overlap(&s, &everything, &parents),
            "`all` missed {s:?}"
        );
    }
}

#[test]
fn an_empty_scope_overlaps_nothing() {
    let mut rng = Rng(0x5eed_0004);
    let parents = index();
    for _ in 0..500 {
        assert!(!scopes_overlap(&[], &scope(&mut rng), &parents));
    }
    assert!(!scopes_overlap(&[], &[ScopeTerm::All], &parents));
}

// -- the conservative bias, tested in both directions (docs/13 P1 deliverables) -----

fn paths(globs: &[&str]) -> Vec<ScopeTerm> {
    globs
        .iter()
        .map(|g| ScopeTerm::Path(Glob::new(g)))
        .collect()
}

#[test]
fn conservative_bias_reports_a_known_false_positive() {
    // docs/02 §10.2: both literal prefixes are `sim`, so the test reports an overlap
    // that does not exist in practice. False positives are the acceptable direction.
    assert!(scopes_overlap(
        &paths(&["sim/*/mod.rs"]),
        &paths(&["sim/tests/**"]),
        &PartOfIndex::empty()
    ));
}

#[test]
fn conservative_bias_never_misses_a_real_overlap() {
    let parents = PartOfIndex::empty();
    for (a, b) in [
        ("sim/**", "sim/src/project/**"),
        ("sim/src/project/**", "sim/**"),
        ("sim/src/step.rs", "sim/src/step.rs"),
        ("sim/**", "sim/*/mod.rs"),
    ] {
        assert!(
            scopes_overlap(&paths(&[a]), &paths(&[b]), &parents),
            "missed {a} vs {b}"
        );
    }
}

#[test]
fn disjoint_trees_do_not_overlap() {
    let parents = PartOfIndex::empty();
    for (a, b) in [
        ("sim/**", "lege/**"),
        ("docs/generated/**", "sim/src/step.rs"),
        ("lege/src/render/**", "sim/**"),
    ] {
        assert!(
            !scopes_overlap(&paths(&[a]), &paths(&[b]), &parents),
            "{a} vs {b} overlapped"
        );
    }
}

#[test]
fn ref_terms_overlap_through_part_of_and_never_with_paths() {
    let parents = index();
    let milestone = vec![ScopeTerm::Ref(Reference::head(
        LogicalKey::parse("sys.milestone.m3-playable-day").expect("valid key"),
    ))];
    let track = vec![ScopeTerm::Ref(Reference::head(
        LogicalKey::parse("sys.track.lighting").expect("valid key"),
    ))];
    let unrelated = vec![ScopeTerm::Ref(Reference::head(
        LogicalKey::parse("sys.track.perf-watch").expect("valid key"),
    ))];

    assert!(
        scopes_overlap(&milestone, &track, &parents),
        "part_of ancestry must overlap"
    );
    assert!(!scopes_overlap(&milestone, &unrelated, &parents));
    // A ref term and a path term never overlap directly: inferring paths from a track's
    // contents would make overlap depend on the whole graph.
    assert!(!scopes_overlap(
        &track,
        &paths(&["content/**/light/**"]),
        &parents
    ));
}
