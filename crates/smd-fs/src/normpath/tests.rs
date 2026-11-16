use super::*;

#[test]
fn normalise_is_identity_key() {
    let k = |s: &str| NormPath::new(s).map(|p| p.as_str().to_owned());
    assert_eq!(k("Common\\Traits\\Foo.TXT").unwrap(), "common/traits/foo.txt");
    assert_eq!(k("e\u{301}.txt").unwrap(), k("\u{e9}.txt").unwrap());
    assert_eq!(k("\u{e9}.txt").unwrap(), "\u{e9}.txt");
    assert_eq!(k("/a/b").unwrap(), "a/b");
    assert_eq!(k("a/../b"), Err(NormError::ParentComponent));
    assert_eq!(k("a//b"), Err(NormError::EmptyComponent));
    assert_eq!(k("a/b/"), Err(NormError::EmptyComponent));
    assert_eq!(k("a/./b"), Err(NormError::CurrentComponent));
    assert_eq!(k(""), Err(NormError::Empty));
    assert_eq!(k("/"), Err(NormError::Empty));
    // Std lowercase, not Turkic: U+0130 -> "i" + U+0307, and plain "I" -> "i".
    assert_eq!(k("\u{130}").unwrap(), "i\u{307}");
    assert_eq!(k("I").unwrap(), "i");
}

#[test]
fn shipped_name_keeps_case_and_is_nfc() {
    assert_eq!(&*shipped_name("Re\u{301}sume.TXT"), "R\u{e9}sume.TXT");
}

#[test]
fn ordering_is_by_bytes() {
    let a = NormPath::new("a/b").unwrap();
    let b = NormPath::new("a0").unwrap();
    assert!(a < b);
}
