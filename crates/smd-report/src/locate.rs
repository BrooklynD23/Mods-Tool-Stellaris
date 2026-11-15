//! The `smd locate` report: where the game lives on this machine and every mod found.
//!
//! Same rules as the scan report: every string is redacted once, then escaped for its
//! sink, and JSON carries strings exactly. The JSON field names are a contract with
//! external consumers; add fields, never rename.

use serde::Serialize;

use crate::escape::{markdown as md, terminal};
use crate::model::{DiagnosticRow, RenderOptions};
use crate::redact::Redactor;
use crate::{DiagnosticsFormat, json};

/// One place location discovery tried.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocateCandidate {
    /// `game_dir`, `user_dir`, `workshop_dir`, `steam_root` or `steam_library`.
    pub key: String,
    /// The path tried.
    pub path: String,
    /// `flag`, `env`, `steam`, `registry` or `default`.
    pub origin: String,
    /// Whether it exists.
    pub exists: bool,
}

/// One mod found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocateMod {
    /// Stable identity.
    pub key: String,
    /// Display name.
    pub name: String,
    /// The mod's own version.
    pub version: Option<String>,
    /// `supported_version` as written.
    pub supported_version: Option<String>,
    /// `workshop`, `local`, ...
    pub role: String,
    /// The launcher `*.mod` file name.
    pub mod_file: Option<String>,
    /// The content directory, when readable.
    pub content_root: Option<String>,
    /// Whether the content may be walked.
    pub readable: bool,
    /// The workshop id.
    pub remote_file_id: Option<String>,
}

/// The renderer input of `smd locate`, as text.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LocateReport {
    /// `steam`, `gog`, `xbox`, `other` or `unknown`.
    pub store: String,
    /// The game install directory.
    pub game_dir: Option<String>,
    /// The user data directory.
    pub user_dir: Option<String>,
    /// Every workshop content directory.
    pub workshop_dirs: Vec<String>,
    /// Every Steam library.
    pub steam_libraries: Vec<String>,
    /// Everything tried, in order.
    pub candidates: Vec<LocateCandidate>,
    /// Every mod found.
    pub mods: Vec<LocateMod>,
    /// Location and discovery diagnostics.
    pub diagnostics: Vec<DiagnosticRow>,
    /// Discovery may have missed mods.
    pub incomplete: bool,
}

#[derive(Serialize)]
struct CandidateView {
    key: String,
    path: String,
    origin: String,
    exists: bool,
}

#[derive(Serialize)]
struct ModView {
    key: String,
    name: String,
    version: Option<String>,
    supported_version: Option<String>,
    role: String,
    mod_file: Option<String>,
    content_root: Option<String>,
    readable: bool,
    remote_file_id: Option<String>,
}

#[derive(Serialize)]
struct DiagView {
    code: String,
    level: String,
    subject: String,
    detail: String,
}

#[derive(Serialize)]
struct LocateView {
    schema: u32,
    store: String,
    game_dir: Option<String>,
    user_dir: Option<String>,
    workshop_dirs: Vec<String>,
    steam_libraries: Vec<String>,
    candidates: Vec<CandidateView>,
    mods: Vec<ModView>,
    diagnostics: Vec<DiagView>,
    incomplete: bool,
}

fn view(r: &LocateReport, x: &Redactor<'_>) -> LocateView {
    let t = |s: &String| x.text(s);
    let o = |s: &Option<String>| s.as_ref().map(|s| x.text(s));
    LocateView {
        schema: 1,
        store: t(&r.store),
        game_dir: o(&r.game_dir),
        user_dir: o(&r.user_dir),
        workshop_dirs: r.workshop_dirs.iter().map(t).collect(),
        steam_libraries: r.steam_libraries.iter().map(t).collect(),
        candidates: r
            .candidates
            .iter()
            .map(|c| CandidateView {
                key: t(&c.key),
                path: t(&c.path),
                origin: t(&c.origin),
                exists: c.exists,
            })
            .collect(),
        mods: r
            .mods
            .iter()
            .map(|m| ModView {
                key: t(&m.key),
                name: x.name(&m.name),
                version: o(&m.version),
                supported_version: o(&m.supported_version),
                role: t(&m.role),
                mod_file: o(&m.mod_file),
                content_root: o(&m.content_root),
                readable: m.readable,
                remote_file_id: o(&m.remote_file_id),
            })
            .collect(),
        diagnostics: r
            .diagnostics
            .iter()
            .map(|d| DiagView {
                code: t(&d.code),
                level: format!("{:?}", d.level).to_lowercase(),
                subject: t(&d.location),
                detail: t(&d.detail),
            })
            .collect(),
        incomplete: r.incomplete,
    }
}

fn table(v: &LocateView) -> String {
    let e = terminal::text;
    let none = |o: &Option<String>| o.as_deref().map_or("(not found)".to_owned(), e);
    let mut out = format!("store: {}\n", e(&v.store));
    out += &format!("game_dir: {}\nuser_dir: {}\n", none(&v.game_dir), none(&v.user_dir));
    out += &format!("workshop_dirs ({}):\n", v.workshop_dirs.len());
    out += &v.workshop_dirs.iter().map(|p| format!("  {}\n", e(p))).collect::<String>();
    out += &format!("steam_libraries ({}):\n", v.steam_libraries.len());
    out += &v.steam_libraries.iter().map(|p| format!("  {}\n", e(p))).collect::<String>();
    out += &format!("candidates ({}):\n", v.candidates.len());
    for c in &v.candidates {
        let mark = if c.exists { "found  " } else { "absent " };
        out += &format!("  {mark}{:<14} {:<9} {}\n", e(&c.key), e(&c.origin), e(&c.path));
    }
    out += &format!("mods ({}):\n", v.mods.len());
    for m in &v.mods {
        let ver = m.version.as_deref().map_or(String::new(), |s| format!(" v{}", e(s)));
        let note = if m.readable { "" } else { "  [content not readable]" };
        out += &format!("  {} [{}] {}{ver}{note}\n", e(&m.key), e(&m.role), e(&m.name));
    }
    for d in &v.diagnostics {
        out += &format!("{} {}: {} {}\n", e(&d.level), e(&d.code), e(&d.subject), e(&d.detail));
    }
    if v.incomplete {
        out += "incomplete: discovery may have missed mods\n";
    }
    out
}

fn markdown(v: &LocateView) -> String {
    let c = md::code_span;
    let none = |o: &Option<String>| o.as_deref().map_or("(not found)".to_owned(), c);
    let mut out = format!("# Locations\n\n- store: {}\n", md::tag(&v.store));
    out += &format!(
        "- game directory: {}\n- user directory: {}\n",
        none(&v.game_dir),
        none(&v.user_dir)
    );
    out += &format!("\n## Workshop directories ({})\n\n", v.workshop_dirs.len());
    out += &v.workshop_dirs.iter().map(|p| format!("- {}\n", c(p))).collect::<String>();
    out += &format!("\n## Candidates tried ({})\n\n", v.candidates.len());
    for k in &v.candidates {
        let mark = if k.exists { "found" } else { "absent" };
        out += &format!("- {mark}: {} ({}) {}\n", md::tag(&k.key), md::tag(&k.origin), c(&k.path));
    }
    out += &format!("\n## Mods ({})\n\n", v.mods.len());
    for m in &v.mods {
        let ver = m.version.as_deref().map_or(String::new(), |s| format!(" v{}", md::text(s)));
        let note = if m.readable { "" } else { " (content not readable)" };
        out +=
            &format!("- {} ({}) {}{ver}{note}\n", c(&m.key), md::tag(&m.role), md::name(&m.name));
    }
    if !v.diagnostics.is_empty() {
        out += "\n## Diagnostics\n\n";
    }
    for d in &v.diagnostics {
        out += &format!(
            "- {} {}: {} {}\n",
            md::tag(&d.level),
            c(&d.code),
            c(&d.subject),
            md::text(&d.detail)
        );
    }
    if v.incomplete {
        out += "\nIncomplete: discovery may have missed mods.\n";
    }
    out
}

/// Render the locate report in `format`, redacted per `opts`.
#[must_use]
pub fn render_locate(
    format: DiagnosticsFormat,
    report: &LocateReport,
    opts: &RenderOptions,
) -> String {
    let x = Redactor::new(&opts.redaction, opts.redact);
    let v = view(report, &x);
    match format {
        DiagnosticsFormat::Table => table(&v),
        DiagnosticsFormat::Markdown => markdown(&v),
        DiagnosticsFormat::Json => json::encode(&v),
    }
}

#[cfg(test)]
mod tests {
    use smd_core::DiagSeverity;

    use super::*;
    use crate::RedactionContext;

    fn sample() -> LocateReport {
        LocateReport {
            store: "steam".into(),
            game_dir: Some("/home/al/lib/game".into()),
            workshop_dirs: vec!["/home/al/w".into()],
            candidates: vec![LocateCandidate {
                key: "game_dir".into(),
                path: "/home/al/lib/game".into(),
                origin: "steam".into(),
                exists: true,
            }],
            mods: vec![LocateMod {
                key: "1".into(),
                name: "Evil \u{1b}[31m`name`".into(),
                version: None,
                supported_version: Some("3.*".into()),
                role: "workshop".into(),
                mod_file: None,
                content_root: Some("/home/al/w/1".into()),
                readable: true,
                remote_file_id: Some("1".into()),
            }],
            diagnostics: vec![DiagnosticRow {
                code: "locate.unresolved".into(),
                level: DiagSeverity::Warn,
                location: "user_dir".into(),
                detail: "none".into(),
            }],
            ..LocateReport::default()
        }
    }

    fn opts(redact: bool) -> RenderOptions {
        let ctx = RedactionContext::from_vars(Some("/home/al".into()), Some("al".into()));
        let o = RenderOptions::new(ctx);
        if redact { o } else { o.without_redaction() }
    }

    #[test]
    fn json_has_the_contracted_fields_and_redacts_unless_told_not_to() {
        let out = render_locate(DiagnosticsFormat::Json, &sample(), &opts(true));
        let j: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(j["schema"], 1);
        assert_eq!(j["game_dir"], "~/lib/game");
        assert!(j["user_dir"].is_null());
        assert_eq!(j["mods"][0]["supported_version"], "3.*");
        assert_eq!(j["mods"][0]["content_root"], "~/w/1");
        assert_eq!(j["diagnostics"][0]["level"], "warn");
        assert_eq!(j["diagnostics"][0]["subject"], "user_dir");
        assert_eq!(j["incomplete"], false);
        let raw: serde_json::Value =
            serde_json::from_str(&render_locate(DiagnosticsFormat::Json, &sample(), &opts(false)))
                .unwrap();
        assert_eq!(raw["game_dir"], "/home/al/lib/game");
        let keys: Vec<&str> = raw.as_object().unwrap().keys().map(String::as_str).collect();
        for k in ["steam_libraries", "candidates", "mods", "workshop_dirs", "store"] {
            assert!(keys.contains(&k), "{k}");
        }
    }

    #[test]
    fn table_and_markdown_keep_control_bytes_inert() {
        for f in [DiagnosticsFormat::Table, DiagnosticsFormat::Markdown] {
            let out = render_locate(f, &sample(), &opts(true));
            assert!(!out.contains('\u{1b}'), "{out}");
            assert!(out.contains("~/lib/game"), "{out}");
            assert!(!out.contains("/home/al"), "{out}");
        }
    }
}
