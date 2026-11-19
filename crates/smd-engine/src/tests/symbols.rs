//! M2 end to end: script files are parsed after the walk and the symbol rules see them.

use smd_core::Role;

use super::fixture::World;
use crate::scan;

#[test]
fn parsed_definitions_reach_the_symbol_rules() {
    let w = World::new();
    w.vanilla("common/buildings/00_v.txt", "b_old = { x = 1 }\nb_keep = { y = 1 }");
    let replace = ("common/buildings/00_v.txt", "b_keep = { y = 1 }");
    w.mod_with("111", "Mod 111", &[replace, ("common/buildings/a.txt", "b_x = { a = 1 }")]);
    w.mod_with("222", "Mod 222", &[("common/buildings/z.txt", "b_x = { a = 2 }")]);
    w.dlc_load(&["111", "222"]);
    let out = scan(&w.request()).expect("scans");
    let find = |rule: &str, subject: &str| {
        out.findings.iter().find(|f| f.rule.as_str() == rule && &*f.subject_text == subject)
    };
    let collision = find("core.symbol_collision", "b_x").expect("b_x collides");
    let winner: Vec<_> = collision
        .participants
        .iter()
        .filter(|p| p.role == Role::Winner)
        .map(|p| &*p.mod_name)
        .collect();
    assert_eq!(winner, ["Mod 222"], "LIOS: z.txt sorts last and wins");
    let deleted = find("core.symbol_deleted", "b_old").expect("b_old is gone");
    assert!(deleted.gaps.is_empty(), "a stated deletion: {:?}", deleted.gaps);
    assert!(find("core.symbol_deleted", "b_keep").is_none());
    assert_eq!(out.completeness.partial, 0);
}
