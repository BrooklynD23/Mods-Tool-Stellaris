//! Reading the active playset, the data invariants, and the logical digest.

use std::collections::BTreeSet;

use rusqlite::Connection;
use rusqlite::types::Value;

use super::schema::Schema;
use super::verify::{Fail, Kind};

/// A position, normalised so integer and zero-padded-string encodings sort alike.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Pos {
    Int(i64),
    Text(String),
}

impl Pos {
    fn canonical(&self) -> String {
        match self {
            Pos::Int(i) => format!("i{i}"),
            Pos::Text(s) => format!("t{s}"),
        }
    }
}

/// One `playsets_mods` row of the active playset.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Entry {
    pub mod_id: String,
    pub pos: Pos,
    pub enabled: bool,
}

/// The `mods` row an entry points at.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct ModRow {
    pub id: String,
    pub steam_id: Option<String>,
    pub dir_path: Option<String>,
    pub display_name: Option<String>,
}

/// The verified content of a snapshot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Rows {
    pub playset_id: String,
    pub playset_name: String,
    /// In ascending position order.
    pub entries: Vec<Entry>,
    pub mods: Vec<ModRow>,
    pub digest: [u8; 32],
}

fn schema_fail(detail: String) -> Fail {
    Fail::new("read_playset", Kind::Schema, detail)
}

fn io(e: &rusqlite::Error) -> Fail {
    Fail::new("read_playset", Kind::Io, e.to_string())
}

/// Booleans are not type-assumed: `0`/`1` and `'true'`/`'false'` are accepted.
pub(super) fn flag(v: &Value, what: &str) -> Result<bool, Fail> {
    match v {
        Value::Integer(0) => Ok(false),
        Value::Integer(1) => Ok(true),
        Value::Text(t) if t.eq_ignore_ascii_case("true") => Ok(true),
        Value::Text(t) if t.eq_ignore_ascii_case("false") => Ok(false),
        other => Err(schema_fail(format!("{what} is {other:?}, not a boolean"))),
    }
}

fn ident(v: &Value, what: &str) -> Result<String, Fail> {
    match v {
        Value::Integer(i) => Ok(i.to_string()),
        Value::Text(t) => Ok(t.clone()),
        other => Err(schema_fail(format!("{what} is {other:?}, not an id"))),
    }
}

/// An integer, or the older zero-padded 10-character string.
fn position(v: &Value) -> Result<Pos, Fail> {
    match v {
        Value::Integer(i) => Ok(Pos::Int(*i)),
        Value::Text(t) if t.len() == 10 && t.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(Pos::Text(t.to_ascii_lowercase()))
        }
        other => Err(schema_fail(format!(
            "position {other:?} is neither an integer nor a 10-character string"
        ))),
    }
}

fn opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) if !t.is_empty() => Some(t.clone()),
        Value::Integer(i) => Some(i.to_string()),
        _ => None,
    }
}

/// Exactly one active, non-removed playset: `(id, name)`.
fn active_playset(conn: &Connection, schema: Schema) -> Result<(String, String), Fail> {
    let removed = if schema.removed { "isRemoved" } else { "0" };
    let sql = format!("SELECT id, isActive, {removed} FROM playsets ORDER BY id");
    let mut st = conn.prepare(&sql).map_err(|e| io(&e))?;
    let all: Vec<[Value; 3]> = st
        .query_map([], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]))
        .and_then(Iterator::collect)
        .map_err(|e| io(&e))?;
    let mut active = Vec::new();
    for [id, is_active, is_removed] in &all {
        if flag(is_active, "playsets.isActive")? && !flag(is_removed, "playsets.isRemoved")? {
            active.push(ident(id, "playsets.id")?);
        }
    }
    match active.as_slice() {
        [one] => Ok((one.clone(), name_of(conn, one))),
        [] => Err(Fail::new("active_playset", Kind::NoActive, "no active, non-removed playset")),
        many => Err(Fail::new(
            "active_playset",
            Kind::MultipleActive,
            format!("{} active playsets: {}", many.len(), many.join(", ")),
        )),
    }
}

/// The playset's display name, when the column exists.
fn name_of(conn: &Connection, id: &str) -> String {
    conn.query_row("SELECT name FROM playsets WHERE CAST(id AS TEXT) = ?1", [id], |r| {
        r.get::<_, String>(0)
    })
    .unwrap_or_default()
}

fn entries(conn: &Connection, playset: &str) -> Result<Vec<Entry>, Fail> {
    let sql =
        "SELECT modId, position, enabled FROM playsets_mods WHERE CAST(playsetId AS TEXT) = ?1";
    let mut st = conn.prepare(sql).map_err(|e| io(&e))?;
    let raw: Vec<[Value; 3]> = st
        .query_map([playset], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]))
        .and_then(Iterator::collect)
        .map_err(|e| io(&e))?;
    let mut out = Vec::with_capacity(raw.len());
    for [m, p, e] in &raw {
        let entry = Entry {
            mod_id: ident(m, "playsets_mods.modId")?,
            pos: position(p)?,
            enabled: flag(e, "playsets_mods.enabled")?,
        };
        out.push(entry);
    }
    out.sort_by(|a, b| (&a.pos, &a.mod_id).cmp(&(&b.pos, &b.mod_id)));
    Ok(out)
}

fn mods(conn: &Connection, schema: Schema) -> Result<Vec<ModRow>, Fail> {
    let col = |present: bool, name: &str| if present { name.to_owned() } else { "NULL".to_owned() };
    let sql = format!(
        "SELECT id, {}, {}, {} FROM mods",
        col(schema.steam_id, "steamId"),
        col(schema.dir_path, "dirPath"),
        col(schema.display_name, "displayName"),
    );
    let mut st = conn.prepare(&sql).map_err(|e| io(&e))?;
    let raw: Vec<[Value; 4]> = st
        .query_map([], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?]))
        .and_then(Iterator::collect)
        .map_err(|e| io(&e))?;
    let mut out = Vec::with_capacity(raw.len());
    for [id, steam, dir, name] in &raw {
        out.push(ModRow {
            id: ident(id, "mods.id")?,
            steam_id: opt_text(steam),
            dir_path: opt_text(dir),
            display_name: opt_text(name),
        });
    }
    Ok(out)
}

fn invariants(entries: &[Entry], mods: &[ModRow]) -> Result<(), Fail> {
    let bad = |d: String| Fail::new("data_invariants", Kind::Invariant, d);
    let kinds: BTreeSet<bool> = entries.iter().map(|e| matches!(e.pos, Pos::Int(_))).collect();
    if kinds.len() > 1 {
        return Err(Fail::new(
            "data_invariants",
            Kind::Schema,
            "positions mix integer and string encodings".to_owned(),
        ));
    }
    if let Some(w) = entries.windows(2).find(|w| w[0].pos == w[1].pos) {
        return Err(bad(format!(
            "position {} is used by more than one entry",
            w[0].pos.canonical()
        )));
    }
    let known: BTreeSet<&str> = mods.iter().map(|m| m.id.as_str()).collect();
    match entries.iter().find(|e| !known.contains(e.mod_id.as_str())) {
        Some(e) => Err(bad(format!("modId {} has no row in mods", e.mod_id))),
        None => Ok(()),
    }
}

/// `blake3` over the active playset id and its `(playset id, mod id, position, enabled)`
/// rows in fixed order.
fn digest(playset: &str, entries: &[Entry]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    let mut field = |s: &str| {
        h.update(&(s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    };
    field("smd-launcher-digest-v1");
    field(playset);
    for e in entries {
        field(playset);
        field(&e.mod_id);
        field(&e.pos.canonical());
        field(if e.enabled { "1" } else { "0" });
    }
    *h.finalize().as_bytes()
}

/// Read and check the active playset on `conn`.
pub(super) fn read(conn: &Connection, schema: Schema) -> Result<Rows, Fail> {
    let (playset_id, playset_name) = active_playset(conn, schema)?;
    let entries = entries(conn, &playset_id)?;
    let mods = mods(conn, schema)?;
    invariants(&entries, &mods)?;
    let digest = digest(&playset_id, &entries);
    Ok(Rows { playset_id, playset_name, entries, mods, digest })
}
