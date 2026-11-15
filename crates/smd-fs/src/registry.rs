//! Windows registry reads without `unsafe`: run `reg.exe query` and parse its text.
//!
//! The parser is pure and tested on every platform. [`query`] spawns the process on
//! Windows and answers nothing elsewhere. Read-only: `query` is the only verb used.

/// One value as `reg.exe query` prints it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RegValue {
    /// The key it lives under, e.g. `HKEY_CURRENT_USER\Software\Valve\Steam`.
    pub key: String,
    /// The value name.
    pub name: String,
    /// The data, unexpanded (`REG_EXPAND_SZ` keeps its `%VARS%`).
    pub data: String,
}

/// Parse `reg.exe query` output into its values.
///
/// A line that starts in column 0 with `HK` opens a key; an indented
/// `<name>    REG_<type>    <data>` line is a value of the open key. Anything else
/// (blank lines, `End of search`, errors) is skipped.
#[must_use]
pub fn parse_reg_output(output: &str) -> Vec<RegValue> {
    let mut key = String::new();
    let mut out = Vec::new();
    for line in output.lines().map(|l| l.trim_end_matches('\r')) {
        if line.starts_with("HK") {
            key = line.trim().to_owned();
            continue;
        }
        let Some(at) = line.find("    REG_").filter(|&at| at > 0 && line.starts_with(' ')) else {
            continue;
        };
        let rest = &line[at + 4..];
        let (_ty, data) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        out.push(RegValue {
            key: key.clone(),
            name: line[..at].trim().to_owned(),
            data: data.trim().to_owned(),
        });
    }
    out
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

/// `reg.exe query <key> [/v <value>] [/s]`; empty off Windows or when the call fails.
#[cfg(windows)]
#[must_use]
pub fn query(key: &str, value: Option<&str>, recursive: bool) -> Vec<RegValue> {
    let mut cmd = std::process::Command::new("reg.exe");
    cmd.arg("query").arg(key);
    if let Some(v) = value {
        cmd.arg("/v").arg(v);
    }
    if recursive {
        cmd.arg("/s");
    }
    match cmd.output() {
        Ok(o) if o.status.success() => parse_reg_output(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

/// `reg.exe query` exists only on Windows.
#[cfg(not(windows))]
#[must_use]
pub fn query(_key: &str, _value: Option<&str>, _recursive: bool) -> Vec<RegValue> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n    Language    REG_SZ    english\r\n\r\nEnd of search: 2 match(es) found.\r\n";

    #[test]
    fn values_parse_with_spaces_in_data() {
        let v = parse_reg_output(OUT);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].key, r"HKEY_CURRENT_USER\Software\Valve\Steam");
        assert_eq!(v[0].name, "SteamPath");
        assert_eq!(v[0].data, "c:/program files (x86)/steam");
    }

    #[test]
    fn recursive_output_keeps_each_values_key() {
        let t = "HKLM\\A\\One\n    DisplayName    REG_SZ    Some Game\nHKLM\\A\\Two\n    DisplayName    REG_SZ    Other\n";
        let v = parse_reg_output(t);
        assert_eq!((v[0].key.as_str(), v[0].data.as_str()), (r"HKLM\A\One", "Some Game"));
        assert_eq!((v[1].key.as_str(), v[1].data.as_str()), (r"HKLM\A\Two", "Other"));
    }

    #[test]
    fn errors_and_junk_yield_nothing() {
        assert!(parse_reg_output("ERROR: The system was unable to find the key.").is_empty());
        assert!(parse_reg_output("").is_empty());
        assert!(parse_reg_output("    REG_SZ only").is_empty());
    }

    #[test]
    fn empty_data_and_expand_sz() {
        let t = "HKCU\\X\n    Personal    REG_EXPAND_SZ    %USERPROFILE%\\Documents\n    E    REG_SZ    \n";
        let v = parse_reg_output(t);
        assert_eq!(v[0].data, r"%USERPROFILE%\Documents");
        assert_eq!(v[1].data, "");
    }

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
