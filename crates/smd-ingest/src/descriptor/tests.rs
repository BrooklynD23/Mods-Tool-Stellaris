use smd_core::{
    Descriptor, DescriptorReader, Diagnostic, PathId, Site, SourceId, Span, VersionComponent,
};

use super::*;

fn site() -> Site {
    Site { source: SourceId(1), path: PathId(0), span: Span::default() }
}

fn read(bytes: &[u8]) -> (Descriptor, Vec<Diagnostic>) {
    KeyValuesReader.read(bytes, site())
}

const FIXTURE: &str = r##"# a comment
name="Example \"Quoted\" Mod"
path="mod/example"
remote_file_id="1234567890"
supported_version="3.13.*"
version="1.0"
tags={
    "Gameplay" "Balance"
    Fixes
}
dependencies={ "UI Overhaul Dynamic" }
replace_path="common/governments/civics"
replace_path="common/traits"
replace_path="history/countries"
picture="thumb.png"
"##;

#[test]
fn descriptor_accumulates_replace_path() {
    let (d, diags) = read(FIXTURE.as_bytes());
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(d.replace_paths.len(), 3);
    let paths: Vec<&str> = d.replace_paths.iter().map(AsRef::as_ref).collect();
    assert_eq!(paths, ["common/governments/civics", "common/traits", "history/countries"]);
    assert_eq!(&*d.name, "Example \"Quoted\" Mod");
    assert_eq!(d.path.as_deref(), Some(std::path::Path::new("mod/example")));
    assert_eq!(d.remote_file_id.as_deref(), Some("1234567890"));
    assert_eq!(d.version.as_deref(), Some("1.0"));
    let tags: Vec<&str> = d.tags.iter().map(AsRef::as_ref).collect();
    assert_eq!(tags, ["Gameplay", "Balance", "Fixes"]);
    assert_eq!(&*d.dependencies[0], "UI Overhaul Dynamic");
    let v = d.supported_version.expect("pattern");
    assert_eq!(
        v.0.as_slice(),
        &[VersionComponent::Exact(3), VersionComponent::Exact(13), VersionComponent::Wildcard]
    );
}

#[test]
fn strings_are_stored_verbatim_control_characters_included() {
    let (d, diags) = read(b"name=\"a\x07b\x1b[31m\\n\"\nversion=\"C:\\mods\\x\\\\\"\n");
    assert!(diags.is_empty());
    assert_eq!(&*d.name, "a\x07b\x1b[31m\\n");
    assert_eq!(d.version.as_deref(), Some("C:\\mods\\x\\\\"));
}

fn invalid(bytes: &[u8]) -> Vec<Diagnostic> {
    let (d, diags) = read(bytes);
    assert_eq!(d, Descriptor::default(), "a capped descriptor is rejected whole");
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code, crate::codes::DESCRIPTOR_INVALID);
    diags
}

#[test]
fn h24_descriptor_caps() {
    // 65 KiB of file.
    let mut big = b"name=\"x\"\n".to_vec();
    big.resize(65 * 1024, b'\n');
    invalid(&big);
    // 5 KiB value.
    invalid(format!("name=\"{}\"", "a".repeat(5 * 1024)).as_bytes());
    invalid(format!("name={}", "a".repeat(5 * 1024)).as_bytes());
    // 257 tags, but 256 is fine.
    let tags = |n: usize| format!("tags={{ {} }}", "\"t\" ".repeat(n));
    invalid(tags(257).as_bytes());
    assert_eq!(read(tags(256).as_bytes()).0.tags.len(), 256);
    let reps = "replace_path=\"p\"\n".repeat(257);
    invalid(reps.as_bytes());
    // Depth 9, but 8 is fine.
    let nest = |n: usize| format!("{}{}", "k={".repeat(n), "}".repeat(n));
    invalid(nest(9).as_bytes());
    assert!(read(nest(8).as_bytes()).1.is_empty());
}

#[test]
fn malformed_text_keeps_what_was_read_before_the_fault() {
    let (d, diags) = read(b"name=\"kept\"\ntags={ \"a\"\n");
    assert_eq!(&*d.name, "kept");
    assert_eq!(diags[0].code, crate::codes::DESCRIPTOR_SYNTAX);
    assert_eq!(read(b"name=\"open").1[0].code, crate::codes::DESCRIPTOR_SYNTAX);
    assert_eq!(read(b"}").1[0].code, crate::codes::DESCRIPTOR_SYNTAX);
    assert_eq!(read(b"= x").1[0].code, crate::codes::DESCRIPTOR_SYNTAX);
}

#[test]
fn h24_descriptor_never_panics() {
    let mut seed = FIXTURE.as_bytes().to_vec();
    while seed.len() < 2048 {
        seed.extend_from_slice(FIXTURE.as_bytes());
    }
    seed.truncate(2048);
    for end in 0..=seed.len() {
        let _ = read(&seed[..end]);
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state =
            state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    for _ in 0..2000 {
        let mut m = seed.clone();
        let at = next() % m.len();
        m[at] = u8::try_from(next() % 256).unwrap_or(0);
        let _ = read(&m);
    }
}

#[test]
fn invalid_utf8_is_flagged_not_silently_replaced() {
    let (d, diags) = read(b"name=\"bad\xFF\"\n");
    assert!(d.name.contains('\u{FFFD}'));
    assert!(diags.iter().any(|x| x.code == crate::codes::DESCRIPTOR_SYNTAX), "{diags:?}");
    let (_, clean) = read(b"name=\"fine\"\n");
    assert!(clean.is_empty());
}
