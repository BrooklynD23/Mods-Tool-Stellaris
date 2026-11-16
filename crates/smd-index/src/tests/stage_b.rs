use smd_core::{Activation, Outcome, SamePath, SourceId};

use super::{X, pid, three_way};
use crate::PlaysetBuilder;

#[test]
fn stage_b_marks_the_stated_winner_only() {
    let (b, _, m) = three_way();
    let ix = b.build();
    let p = pid(&ix, X);
    let d = ix.derived();
    assert!(d.is_surviving(ix.overlay().slot_of(p, m).unwrap()));
    assert_eq!(d.surviving().count(), 1);
    assert_eq!(d.possibly().count(), 0);
    assert!(d.path_gaps(p).is_empty());
}

#[test]
fn directory_scan_all_possibly_surviving() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.file(SourceId::VANILLA, X, "v").file(a, X, "a").file(m, X, "b").file(a, "solo.txt", "s");
    b.directory_scan();
    let ix = b.build();
    let p = pid(&ix, X);
    for s in [a, m] {
        let slot = ix.overlay().slot_of(p, s).unwrap();
        assert!(!ix.derived().is_surviving(slot));
        assert!(ix.derived().is_possibly(slot));
    }
    assert!(!ix.derived().path_gaps(p).is_empty());
    let solo = pid(&ix, "solo.txt");
    assert!(ix.derived().is_possibly(ix.overlay().slot_of(solo, a).unwrap()));
}

/// xorshift, so the loop is deterministic with no dependency.
fn next(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

#[test]
fn stage_b_invariants_hold_over_random_playsets() {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..200 {
        let mut b = PlaysetBuilder::new();
        let n = 1 + (next(&mut seed) % 4) as usize;
        let mut ids = vec![SourceId::VANILLA];
        for i in 0..n {
            let act = [Activation::Active, Activation::Inactive, Activation::Unknown]
                [(next(&mut seed) % 3) as usize];
            let rank: Vec<u32> = if next(&mut seed) % 4 == 0 { vec![] } else { vec![2, i as u32] };
            ids.push(b.add_mod(&format!("m{i}"), &rank, act));
        }
        if next(&mut seed) % 3 == 0 {
            b.same_path(SamePath::Unknown);
        }
        for f in 0..4 {
            for &s in &ids {
                if next(&mut seed) % 2 == 0 {
                    b.file(s, &format!("d/f{f}.txt"), &format!("c{}", next(&mut seed) % 2));
                }
            }
        }
        if next(&mut seed) % 2 == 0 {
            b.replace_path(ids[ids.len() - 1], "d");
        }
        let ix = b.build();
        let d = ix.derived();
        assert!(!d.surviving().intersects(d.possibly()));
        for p in ix.overlay().paths() {
            let v = ix.path_verdict(p);
            if let Some(w) = v.resolution.winner {
                assert_eq!(v.resolution.outcome, Outcome::SingleWinner);
                assert!(d.is_surviving(ix.overlay().slot_of(p, w.source).unwrap()));
            } else if !v.resolution.gaps.is_empty() {
                // R2: a blocked resolution is Unknown / Undetermined.
                assert_eq!(v.resolution.outcome, Outcome::Unknown);
                assert_eq!(v.resolution.mechanism, smd_core::Mechanism::Undetermined);
            }
            for c in &v.live {
                let slot = ix.overlay().slot_of(p, c.source).unwrap();
                assert_eq!(
                    d.contends(slot),
                    v.resolution.winner.is_some_and(|w| w == *c) || v.resolution.winner.is_none()
                );
            }
        }
    }
}
