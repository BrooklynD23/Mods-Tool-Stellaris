//! The schema probe: the table and column set the reader depends on.
//!
//! **Assumed, not verified.** The set below is Irony's `v4` launcher model (VERIFY V03,
//! V41); no source describes the launcher that ships with current game builds. The probe
//! *detects* a changed schema; it cannot detect a column whose meaning changed. It never
//! claims the schema matches a real launcher.

use rusqlite::Connection;

use super::verify::{Fail, Kind};

/// Columns the reader requires, per table. `position`, `enabled` and `isActive` are also
/// type-checked.
const REQUIRED: &[(&str, &[&str])] = &[
    ("playsets", &["id", "isActive"]),
    ("playsets_mods", &["playsetId", "modId", "position", "enabled"]),
    ("mods", &["id"]),
];
/// Columns whose declared type must not be `REAL` or `BLOB`.
const TYPED: &[(&str, &str)] =
    &[("playsets", "isActive"), ("playsets_mods", "position"), ("playsets_mods", "enabled")];
/// Columns that identify a mod; at least one must exist or no entry can be matched.
const IDENTIFYING: &[&str] = &["steamId", "dirPath", "displayName", "name"];

/// Optional columns the probe found.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Schema {
    /// `playsets.isRemoved` exists. Its absence means the older schema, not an error.
    pub removed: bool,
    pub steam_id: bool,
    pub dir_path: bool,
    pub display_name: bool,
}

type Table = (String, Vec<(String, String)>);

fn tables(conn: &Connection) -> rusqlite::Result<Vec<Table>> {
    let mut names =
        conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
    let names: Vec<String> = names.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for name in names.into_iter().take(64) {
        let quoted = format!("PRAGMA table_info(\"{}\")", name.replace('"', "\"\""));
        let mut st = conn.prepare(&quoted)?;
        let cols = st
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .take(64)
            .collect::<Result<Vec<_>, _>>()?;
        out.push((name, cols));
    }
    Ok(out)
}

fn describe(found: &[Table]) -> String {
    let each = found.iter().map(|(t, cols)| {
        let cols: Vec<String> =
            cols.iter().map(|(n, ty)| format!("{n} {ty}").trim().to_owned()).collect();
        format!("{t}({})", cols.join(", "))
    });
    each.collect::<Vec<_>>().join("; ")
}

fn column<'a>(found: &'a [Table], table: &str, col: &str) -> Option<&'a str> {
    let (_, cols) = found.iter().find(|(t, _)| t.eq_ignore_ascii_case(table))?;
    cols.iter().find(|(n, _)| n.eq_ignore_ascii_case(col)).map(|(_, ty)| ty.as_str())
}

/// A declared type that no integer, boolean or text value fits.
fn retyped(decl: &str) -> bool {
    let up = decl.to_ascii_uppercase();
    ["BLOB", "REAL", "FLOA", "DOUB"].iter().any(|t| up.contains(t))
}

fn problems(found: &[Table]) -> Vec<String> {
    let mut out = Vec::new();
    for (table, cols) in REQUIRED {
        for col in *cols {
            if column(found, table, col).is_none() {
                out.push(format!("missing column {table}.{col}"));
            }
        }
    }
    for (table, col) in TYPED {
        if let Some(ty) = column(found, table, col).filter(|t| retyped(t)) {
            out.push(format!("column {table}.{col} is declared {ty}"));
        }
    }
    if !IDENTIFYING.iter().any(|c| column(found, "mods", c).is_some()) {
        out.push("mods has none of steamId, dirPath, displayName, name".to_owned());
    }
    out
}

/// Probe `conn`. `Err` names what was missing and what was found.
pub(super) fn probe(conn: &Connection) -> Result<Schema, Fail> {
    let found = tables(conn).map_err(|e| Fail::new("schema_probe", Kind::Io, e.to_string()))?;
    let bad = problems(&found);
    if !bad.is_empty() {
        let detail = format!("{}; found: {}", bad.join(", "), describe(&found));
        return Err(Fail::new("schema_probe", Kind::Schema, detail));
    }
    let has = |c: &str| column(&found, "mods", c).is_some();
    Ok(Schema {
        removed: column(&found, "playsets", "isRemoved").is_some(),
        steam_id: has("steamId"),
        dir_path: has("dirPath"),
        display_name: has("displayName"),
    })
}
