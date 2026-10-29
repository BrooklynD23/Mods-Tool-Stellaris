//! Redaction (doc 14, Privacy; gate H26): a final pass over everything a report prints.
//!
//! It runs once, over every string of the view model, before any sink escapes anything, so
//! it sees the same strings regardless of format (a JSON-escaped Windows path would
//! otherwise defeat it). It replaces the home directory prefix with `~` (case-insensitive),
//! the account name wherever it appears with `<user>`, and the digits of a
//! `userdata/<digits>` Steam account id with `<id>`. Mod display names keep the account
//! name (they are the author's); everything else, including mod keys and free text such as
//! an activation basis, gets the full treatment. A very short account name over-redacts;
//! that is the safe direction.

use crate::view::{DiagView, View};

/// What is personal on this machine.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RedactionContext {
    /// The home directory, e.g. `/home/alice` or `C:\Users\alice`. Empty means none.
    pub home_dir: String,
    /// The OS account name. Empty means none.
    pub account_name: String,
    /// Steam account ids known to be on this machine (digits).
    pub steam_ids: Vec<String>,
}

/// The replacement for the account name.
pub(crate) const USER_PLACEHOLDER: &str = "<user>";
/// The replacement for a Steam account id.
pub(crate) const ID_PLACEHOLDER: &str = "<id>";

impl RedactionContext {
    /// The context of the machine running the process: `HOME` or `USERPROFILE` for the
    /// home directory, `USER` or `USERNAME` for the account (falling back to the home
    /// directory's last component). Empty values count as unset.
    #[must_use]
    pub fn from_env() -> RedactionContext {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        RedactionContext::from_vars(
            var("HOME").or_else(|| var("USERPROFILE")),
            var("USER").or_else(|| var("USERNAME")),
        )
    }

    /// Build a context from an optional home directory and account name.
    #[must_use]
    pub fn from_vars(home: Option<String>, user: Option<String>) -> RedactionContext {
        let home_dir = home.unwrap_or_default();
        let account_name = user.unwrap_or_else(|| {
            home_dir
                .trim_end_matches(['/', '\\'])
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or_default()
                .to_owned()
        });
        RedactionContext { home_dir, account_name, steam_ids: Vec::new() }
    }
}

/// Applies a [`RedactionContext`], or does nothing when redaction is off.
pub(crate) struct Redactor<'a> {
    ctx: Option<&'a RedactionContext>,
}

impl<'a> Redactor<'a> {
    pub(crate) fn new(ctx: &'a RedactionContext, enabled: bool) -> Redactor<'a> {
        Redactor { ctx: enabled.then_some(ctx) }
    }

    /// Redact free text that may carry a path, an OS error message, or an account name.
    pub(crate) fn text(&self, s: &str) -> String {
        self.apply(s, true)
    }

    /// Redact a mod display name: like [`Self::text`] but the account name is kept, since a
    /// name is the author's and not personal.
    pub(crate) fn name(&self, s: &str) -> String {
        self.apply(s, false)
    }

    fn apply(&self, s: &str, account: bool) -> String {
        let Some(ctx) = self.ctx else { return s.to_owned() };
        let mut out = replace_home(s, &ctx.home_dir);
        out = redact_userdata(&out);
        for id in ctx.steam_ids.iter().filter(|id| !id.is_empty()) {
            out = replace_ci(&out, id, ID_PLACEHOLDER);
        }
        if account && !ctx.account_name.is_empty() {
            out = replace_ci(&out, &ctx.account_name, USER_PLACEHOLDER);
        }
        out
    }
}

impl Redactor<'_> {
    /// Redact every string of the view in place. The single place redaction happens for a
    /// report: no field is exempt by being forgotten.
    pub(crate) fn view(&self, v: &mut View) {
        let h = &mut v.header;
        self.string(&mut h.order_source);
        self.string(&mut h.snapshot_method);
        for u in &mut h.unparsed {
            self.string(&mut u.reason);
        }
        h.domains.iter_mut().for_each(|d| self.string(d));
        for c in &mut h.content_incomplete {
            self.string(&mut c.mod_key);
            self.string(&mut c.reason);
            if let Some(d) = &mut c.detail {
                self.string(d);
            }
        }
        for m in &mut h.inventory {
            self.string(&mut m.mod_key);
            m.name = self.name(&m.name);
            self.string(&mut m.role);
            self.string(&mut m.activation);
            self.string(&mut m.activation_basis);
            self.string(&mut m.position);
            if let Some(ver) = &mut m.version {
                self.string(ver);
            }
            self.string(&mut m.path);
            m.descriptor_diagnostics.iter_mut().for_each(|d| self.string(d));
        }
        for f in &mut v.findings {
            self.string(&mut f.rule);
            self.string(&mut f.subject);
            for p in &mut f.participants {
                self.string(&mut p.mod_key);
                p.mod_name = self.name(&p.mod_name);
                self.string(&mut p.path);
            }
            for (k, val) in &mut f.facts {
                self.string(k);
                self.string(val);
            }
        }
        self.diagnostics(&mut v.diagnostics);
    }

    /// Redact the diagnostics rows in place.
    pub(crate) fn diagnostics(&self, rows: &mut [DiagView]) {
        for d in rows {
            self.string(&mut d.level);
            self.string(&mut d.code);
            self.string(&mut d.location);
            self.string(&mut d.detail);
        }
    }

    fn string(&self, s: &mut String) {
        if self.ctx.is_some() {
            *s = self.text(s);
        }
    }
}

/// Replace every ASCII-case-insensitive occurrence of `needle` in `s` with `with`.
/// Windows profile paths differ in case from the environment's, and over-redacting is the
/// safe direction. ASCII lowercasing keeps byte offsets, so slicing `s` is sound.
fn replace_ci(s: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return s.to_owned();
    }
    let hay = s.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (at, _) in hay.match_indices(&needle) {
        if at < last {
            continue;
        }
        out.push_str(&s[last..at]);
        out.push_str(with);
        last = at + needle.len();
    }
    out.push_str(&s[last..]);
    out
}

/// Replace every occurrence of the home directory (either separator style, trailing
/// separator ignored, case-insensitive) with `~`.
fn replace_home(s: &str, home: &str) -> String {
    let home = home.trim_end_matches(['/', '\\']);
    if home.is_empty() {
        return s.to_owned();
    }
    let mut out = replace_ci(s, home, "~");
    let flipped =
        if home.contains('\\') { home.replace('\\', "/") } else { home.replace('/', "\\") };
    if flipped != home {
        out = replace_ci(&out, &flipped, "~");
    }
    out
}

/// `userdata/<digits>` becomes `userdata/<id>`, for either separator.
fn redact_userdata(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.to_ascii_lowercase().find("userdata") {
        let (head, tail) = rest.split_at(at + "userdata".len());
        out.push_str(head);
        let mut chars = tail.chars();
        match chars.next() {
            Some(sep @ ('/' | '\\')) => {
                let digits = chars.as_str().chars().take_while(char::is_ascii_digit).count();
                out.push(sep);
                if digits > 0 {
                    out.push_str(ID_PLACEHOLDER);
                }
                rest = &tail[sep.len_utf8() + digits..];
            }
            _ => rest = tail,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RedactionContext {
        RedactionContext {
            home_dir: "/home/zq".into(),
            account_name: "zq".into(),
            steam_ids: vec!["42".into()],
        }
    }

    #[test]
    fn home_prefix_account_and_userdata_are_replaced() {
        let c = ctx();
        let r = Redactor::new(&c, true);
        assert_eq!(r.text("/home/zq/.steam/userdata/123/config"), "~/.steam/userdata/<id>/config");
        assert_eq!(r.text("C:\\x\\userdata\\9\\y"), "C:\\x\\userdata\\<id>\\y");
        assert_eq!(r.text("/mnt/zq/compat/zq"), "/mnt/<user>/compat/<user>");
        assert_eq!(r.text("id 42 here"), "id <id> here");
    }

    #[test]
    fn windows_paths_match_case_insensitively() {
        let c = RedactionContext {
            home_dir: "C:\\Users\\Alice".into(),
            account_name: "Alice".into(),
            steam_ids: Vec::new(),
        };
        let r = Redactor::new(&c, true);
        assert_eq!(r.text("c:\\users\\alice\\Documents\\x"), "~\\Documents\\x");
        assert_eq!(r.text("c:/USERS/ALICE/x ALICE"), "~/x <user>");
        assert_eq!(r.name("ALICE mod"), "ALICE mod");
    }

    #[test]
    fn env_vars_build_a_context() {
        let c = RedactionContext::from_vars(Some("/home/bob/".into()), None);
        assert_eq!((c.home_dir.as_str(), c.account_name.as_str()), ("/home/bob/", "bob"));
        assert_eq!(Redactor::new(&c, true).text("/home/bob/mods/x"), "~/mods/x");
    }

    #[test]
    fn disabled_redaction_is_the_identity() {
        let c = ctx();
        assert_eq!(Redactor::new(&c, false).text("/home/zq/x"), "/home/zq/x");
    }

    #[test]
    fn an_empty_context_changes_nothing() {
        let c = RedactionContext::default();
        assert_eq!(Redactor::new(&c, true).text("/home/x/userdata"), "/home/x/userdata");
    }
}
