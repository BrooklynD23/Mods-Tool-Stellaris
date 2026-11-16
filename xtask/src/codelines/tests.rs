use super::*;

#[test]
fn doc_comments_and_blanks_do_not_count() {
    let src = "//! module doc\n\n/// item doc\npub fn a() {}\n// trailing note\n";
    let lines = code_lines(src);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "pub fn a() {}");
    assert_eq!(lines[0].number, 4);
}

#[test]
fn block_comments_are_stripped_across_lines() {
    let src = "pub fn a() {}\n/* one\n   two\n   three */\npub fn b() {}\n";
    let lines = code_lines(src);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].text, "pub fn b() {}");
}

#[test]
fn cfg_test_blocks_do_not_count() {
    let src = concat!(
        "pub fn a() {}\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    #[test]\n",
        "    fn t() { assert!(true); }\n",
        "}\n",
        "pub fn b() {}\n",
    );
    let lines = code_lines(src);
    assert_eq!(lines.len(), 2, "got {:?}", lines.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert_eq!(lines[1].text, "pub fn b() {}");
}

#[test]
fn a_cfg_test_module_declaration_costs_one_skipped_line() {
    let src = "pub fn a() {}\n#[cfg(test)]\nmod tests;\npub fn b() {}\n";
    let lines = code_lines(src);
    assert_eq!(lines.len(), 2);
}

#[test]
fn braces_inside_string_literals_do_not_confuse_the_skipper() {
    let src = concat!(
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    fn t() { let _ = \"}}}}\"; }\n",
        "}\n",
        "pub fn after() {}\n",
    );
    let lines = code_lines(src);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "pub fn after() {}");
}

#[test]
fn a_data_only_file_is_exempt_and_a_code_file_is_not() {
    let data = code_lines("pub const A: [u8; 2] = [\n    1, 2,\n];\nstatic B: u8 = 3;\n");
    assert!(is_all_const_or_static(&data));

    let code = code_lines("pub const A: u8 = 1;\npub fn f() {}\n");
    assert!(!is_all_const_or_static(&code));

    let neither = code_lines("use std::fmt;\n");
    assert!(!is_all_const_or_static(&neither), "no data means nothing to exempt");
}
