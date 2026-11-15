//! Windows registry reads without `unsafe`, through Microsoft's `windows-registry` crate
//! (no subprocess, so no OEM-codepage decoding, PATH lookup or hang). [`query`] answers
//! nothing off Windows. Read-only.

/// One registry value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RegValue {
    /// The key it lives under, e.g. `HKEY_CURRENT_USER\Software\Valve\Steam`.
    pub key: String,
    /// The value name.
    pub name: String,
    /// The data; `REG_EXPAND_SZ` is already expanded.
    pub data: String,
}

/// Expand `%NAME%` references with `var`. An unknown name is left as written.
#[must_use]
pub fn expand_percent_vars(s: &str, var: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('%') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('%') {
            Some(close) if close > 0 => match var(&after[..close]) {
                Some(v) => {
                    out.push_str(&v);
                    rest = &after[close + 1..];
                }
                None => {
                    out.push('%');
                    rest = after;
                }
            },
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The values under `key` (`HKCU\\...` / `HKLM\\...`), optionally just `value`, optionally
/// the whole subtree; empty off Windows or when nothing reads. Never writes.
#[cfg(windows)]
#[must_use]
pub fn query(key: &str, value: Option<&str>, recursive: bool) -> Vec<RegValue> {
    let mut out = Vec::new();
    walk(key, value, if recursive { MAX_DEPTH } else { 0 }, &mut out);
    out
}

/// Subkey levels walked below the start key; an uninstall root needs one.
#[cfg(windows)]
const MAX_DEPTH: u32 = 3;

#[cfg(windows)]
fn open_key(key: &str) -> Option<windows_registry::Key> {
    let (root, sub) = key.split_once('\\').unwrap_or((key, ""));
    let hive = match root.to_ascii_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => windows_registry::CURRENT_USER,
        "HKLM" | "HKEY_LOCAL_MACHINE" => windows_registry::LOCAL_MACHINE,
        _ => return None,
    };
    hive.open(sub).ok()
}

/// A string value as text; `REG_EXPAND_SZ` has its `%VARS%` expanded from the
/// environment. Anything that is not text, or not valid UTF-16, is absent.
#[cfg(windows)]
fn text_of(v: windows_registry::Value) -> Option<String> {
    let expand = v.ty() == windows_registry::Type::ExpandString;
    let s = String::try_from(v).ok()?;
    Some(if expand { expand_percent_vars(&s, &|n| std::env::var(n).ok()) } else { s })
}

#[cfg(windows)]
fn walk(key: &str, value: Option<&str>, depth: u32, out: &mut Vec<RegValue>) {
    let Some(open) = open_key(key) else { return };
    let mut found: Vec<(String, windows_registry::Value)> = Vec::new();
    match value {
        Some(n) => found.extend(open.get_value(n).ok().map(|v| (n.to_owned(), v))),
        None => found.extend(open.values().into_iter().flatten()),
    }
    for (name, v) in found {
        if let Some(data) = text_of(v) {
            out.push(RegValue { key: key.to_owned(), name, data });
        }
    }
    if depth > 0 {
        for sub in open.keys().into_iter().flatten() {
            walk(&format!("{key}\\{sub}"), value, depth - 1, out);
        }
    }
}

/// The registry exists only on Windows.
#[cfg(not(windows))]
#[must_use]
pub fn query(_key: &str, _value: Option<&str>, _recursive: bool) -> Vec<RegValue> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_vars_expand_and_unknowns_stay() {
        let var = |n: &str| (n == "USERPROFILE").then(|| r"C:\Users\a".to_owned());
        assert_eq!(expand_percent_vars(r"%USERPROFILE%\Documents", &var), r"C:\Users\a\Documents");
        assert_eq!(expand_percent_vars("%NOPE%/x", &var), "%NOPE%/x");
        assert_eq!(expand_percent_vars("100%", &var), "100%");
        assert_eq!(expand_percent_vars("a%%b", &var), "a%%b");
    }

    #[test]
    #[cfg(not(windows))]
    fn query_is_inert_off_windows() {
        assert!(query(r"HKCU\Software", Some("x"), true).is_empty());
    }
}
