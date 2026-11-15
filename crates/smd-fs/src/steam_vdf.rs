//! Valve KeyValues (VDF) reading, for `libraryfolders.vdf` and `appmanifest_<id>.acf`.
//!
//! The text is untrusted (doc 05): it is capped at [`FILE_MAX`] bytes, nesting at
//! [`DEPTH_MAX`] and library count at [`LIBRARIES_MAX`], and every cap that trips is an
//! error, never a silent truncation.

/// Largest file read, in bytes.
pub const FILE_MAX: usize = 1 << 20;
/// Deepest block nesting accepted.
pub const DEPTH_MAX: usize = 16;
/// Most library entries accepted.
pub const LIBRARIES_MAX: usize = 64;

/// Why a VDF text was rejected.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VdfError {
    /// Over [`FILE_MAX`].
    TooLarge(usize),
    /// Nested deeper than [`DEPTH_MAX`].
    TooDeep,
    /// More than [`LIBRARIES_MAX`] libraries.
    TooManyLibraries,
    /// Not KeyValues text.
    Syntax(&'static str),
}

impl std::fmt::Display for VdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VdfError::TooLarge(n) => write!(f, "{n} bytes is over the {FILE_MAX}-byte cap"),
            VdfError::TooDeep => write!(f, "nested deeper than {DEPTH_MAX} levels"),
            VdfError::TooManyLibraries => write!(f, "more than {LIBRARIES_MAX} libraries"),
            VdfError::Syntax(why) => write!(f, "not valid KeyValues text: {why}"),
        }
    }
}

impl std::error::Error for VdfError {}

enum Node {
    Str(String),
    Map(Vec<(String, Node)>),
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip(&mut self) {
        loop {
            while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.peek().is_some_and(|c| c != b'\n') {
                    self.i += 1;
                }
            } else {
                return;
            }
        }
    }

    fn string(&mut self) -> Result<String, VdfError> {
        let mut out = Vec::new();
        if self.peek() == Some(b'"') {
            self.i += 1;
            loop {
                let c = self.peek().ok_or(VdfError::Syntax("unterminated string"))?;
                self.i += 1;
                match c {
                    b'"' => break,
                    b'\\' => out.push(self.escape()?),
                    c => out.push(c),
                }
            }
        } else {
            while let Some(c) = self.peek() {
                if c.is_ascii_whitespace() || matches!(c, b'{' | b'}' | b'"') {
                    break;
                }
                out.push(c);
                self.i += 1;
            }
            if out.is_empty() {
                return Err(VdfError::Syntax("expected a string"));
            }
        }
        String::from_utf8(out).map_err(|_| VdfError::Syntax("not UTF-8"))
    }

    /// One escape after a backslash. An unknown escape keeps the backslash: some writers
    /// do not escape Windows paths, and `D:\Lib` must survive.
    fn escape(&mut self) -> Result<u8, VdfError> {
        let c = self.peek().ok_or(VdfError::Syntax("unterminated string"))?;
        let out = match c {
            b'\\' | b'"' => c,
            b'n' => b'\n',
            b't' => b'\t',
            _ => return Ok(b'\\'),
        };
        self.i += 1;
        Ok(out)
    }

    fn map(&mut self, depth: usize) -> Result<Vec<(String, Node)>, VdfError> {
        let mut entries = Vec::new();
        loop {
            self.skip();
            match self.peek() {
                None if depth == 0 => return Ok(entries),
                None => return Err(VdfError::Syntax("unclosed block")),
                Some(b'}') if depth > 0 => {
                    self.i += 1;
                    return Ok(entries);
                }
                Some(b'}') => return Err(VdfError::Syntax("unmatched }")),
                Some(_) => {}
            }
            let key = self.string()?;
            self.skip();
            if self.peek() == Some(b'{') {
                if depth >= DEPTH_MAX {
                    return Err(VdfError::TooDeep);
                }
                self.i += 1;
                entries.push((key, Node::Map(self.map(depth + 1)?)));
            } else {
                entries.push((key, Node::Str(self.string()?)));
            }
        }
    }
}

fn parse(text: &str) -> Result<Vec<(String, Node)>, VdfError> {
    if text.len() > FILE_MAX {
        return Err(VdfError::TooLarge(text.len()));
    }
    Parser { s: text.as_bytes(), i: 0 }.map(0)
}

fn find<'a>(entries: &'a [(String, Node)], key: &str) -> Option<&'a Node> {
    entries.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

fn text_of(node: Option<&Node>) -> Option<&String> {
    match node {
        Some(Node::Str(s)) => Some(s),
        _ => None,
    }
}

/// The library paths named by a `libraryfolders.vdf`, in file order.
///
/// Reads both shapes: the old flat `"1" "D:\\Lib"` and the new nested
/// `"0" { "path" "..." }`. Non-numeric keys (stats counters) are ignored.
///
/// # Errors
/// [`VdfError`] when the text is over a cap or not KeyValues.
pub fn parse_library_folders(text: &str) -> Result<Vec<String>, VdfError> {
    let root = parse(text)?;
    let Some(Node::Map(libs)) = find(&root, "libraryfolders") else {
        return Err(VdfError::Syntax("no libraryfolders block"));
    };
    let mut out = Vec::new();
    for (_, value) in libs.iter().filter(|(k, _)| k.bytes().all(|b| b.is_ascii_digit())) {
        let path = match value {
            Node::Str(p) => Some(p),
            Node::Map(m) => text_of(find(m, "path")),
        };
        out.extend(path.filter(|p| !p.is_empty()).cloned());
        if out.len() > LIBRARIES_MAX {
            return Err(VdfError::TooManyLibraries);
        }
    }
    Ok(out)
}

/// The `installdir` of an `appmanifest_<id>.acf`, when it has one.
///
/// # Errors
/// [`VdfError`] when the text is over a cap or not KeyValues.
pub fn parse_install_dir(text: &str) -> Result<Option<String>, VdfError> {
    let root = parse(text)?;
    let Some(Node::Map(app)) = find(&root, "AppState") else {
        return Err(VdfError::Syntax("no AppState block"));
    };
    Ok(text_of(find(app, "installdir")).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEW: &str = r#"
"libraryfolders"
{
    "contentstatsid"    "123"
    "0"
    {
        "path"      "C:\\Program Files (x86)\\Steam"
        "apps" { "281990" "100" }
    }
    "1" { "path" "D:\\Lib" }
}
"#;

    #[test]
    fn new_nested_format_unescapes_backslashes() {
        let libs = parse_library_folders(NEW).unwrap();
        assert_eq!(libs, [r"C:\Program Files (x86)\Steam", r"D:\Lib"]);
    }

    #[test]
    fn old_flat_format() {
        let t = "\"LibraryFolders\"\n{\n\t\"TimeNextStatsReport\"\t\"99\"\n\t\"1\"\t\"D:\\\\Lib\"\n\t\"2\"\t\"/mnt/x\"\n}\n";
        assert_eq!(parse_library_folders(t).unwrap(), [r"D:\Lib", "/mnt/x"]);
    }

    #[test]
    fn comments_unquoted_tokens_and_lone_backslashes_read() {
        let t = "// hi\nlibraryfolders { 0 { path \"/a\" } 1 { path \"D:\\Lib\" } }";
        assert_eq!(parse_library_folders(t).unwrap(), ["/a", r"D:\Lib"]);
    }

    #[test]
    fn garbage_is_rejected() {
        for bad in ["", "}", "\"libraryfolders\" {", "\"a\" \"b", "\"libraryfolders\" { \"0\" }"] {
            assert!(parse_library_folders(bad).is_err(), "{bad:?}");
        }
        assert!(matches!(parse_library_folders("\"x\" \"y\""), Err(VdfError::Syntax(_))));
    }

    #[test]
    fn caps_are_loud() {
        let big = format!("\"a\" \"{}\"", "x".repeat(FILE_MAX));
        assert!(matches!(parse_library_folders(&big), Err(VdfError::TooLarge(_))));
        let deep = format!("{}{}", "\"a\" {".repeat(DEPTH_MAX + 1), "}".repeat(DEPTH_MAX + 1));
        assert_eq!(parse_library_folders(&deep), Err(VdfError::TooDeep));
        let list = |n: usize| -> String {
            let body: String = (0..n).map(|i| format!("\"{i}\" \"/l{i}\"\n")).collect();
            format!("\"libraryfolders\" {{ {body} }}")
        };
        assert_eq!(
            parse_library_folders(&list(LIBRARIES_MAX + 1)),
            Err(VdfError::TooManyLibraries)
        );
        assert_eq!(parse_library_folders(&list(LIBRARIES_MAX)).unwrap().len(), LIBRARIES_MAX);
    }

    #[test]
    fn install_dir_from_manifest() {
        let t = "\"AppState\"\n{\n\t\"appid\"\t\"1\"\n\t\"installdir\"\t\"Some Game\"\n}\n";
        assert_eq!(parse_install_dir(t).unwrap().as_deref(), Some("Some Game"));
        assert_eq!(parse_install_dir("\"AppState\" { }").unwrap(), None);
        assert!(parse_install_dir("junk {{").is_err());
    }
}
