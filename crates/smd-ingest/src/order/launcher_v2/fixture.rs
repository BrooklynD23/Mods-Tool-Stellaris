//! Synthesised launcher databases for tests. No binary fixture is checked in, and none of
//! this claims to match a real launcher: it is Irony's `v4` model as VERIFY V03 records it.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Which `position` encoding the fixture uses.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum PosKind {
    Int,
    PaddedText,
}

const MODS: &str = "CREATE TABLE mods (id TEXT PRIMARY KEY, name TEXT, displayName TEXT, \
    dirPath TEXT, archivePath TEXT, steamId TEXT, status TEXT);";
const PLAYSETS: &str = "CREATE TABLE playsets (id TEXT PRIMARY KEY, name TEXT, isActive BOOLEAN, \
    isRemoved BOOLEAN, loadOrder INTEGER, createdOn TEXT);";

fn join_table(kind: PosKind) -> String {
    let ty = if kind == PosKind::Int { "INTEGER" } else { "TEXT" };
    format!(
        "CREATE TABLE playsets_mods (playsetId TEXT, modId TEXT, position {ty}, enabled BOOLEAN);\
         CREATE INDEX playsets_mods_pos ON playsets_mods (playsetId, position);"
    )
}

/// A launcher database being written, with its writer connection held open (and so the
/// `-wal` and `-shm` files in place) until dropped.
pub(crate) struct Launcher {
    pub conn: Connection,
    pub path: PathBuf,
    kind: PosKind,
}

fn pos_sql(kind: PosKind, n: usize) -> String {
    match kind {
        PosKind::Int => format!("{}", (n + 1) * 10),
        PosKind::PaddedText => format!("'{:010x}'", (n + 1) * 10),
    }
}

impl Launcher {
    /// A WAL-mode database with the default schema and autocheckpoint off.
    pub(crate) fn create(dir: &Path, kind: PosKind) -> Launcher {
        let path = dir.join(super::FILE);
        let conn = Connection::open(&path).expect("create db");
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
            .expect("wal");
        conn.execute_batch(&format!("{MODS}{PLAYSETS}{}", join_table(kind))).expect("schema");
        Launcher { conn, path, kind }
    }

    /// Replace the schema's join table with `ddl`, for schema-probe tests.
    pub(crate) fn with_join_table(dir: &Path, ddl: &str) -> Launcher {
        let path = dir.join(super::FILE);
        let conn = Connection::open(&path).expect("create db");
        conn.execute_batch("PRAGMA journal_mode = WAL;").expect("wal");
        conn.execute_batch(&format!("{MODS}{PLAYSETS}{ddl}")).expect("schema");
        Launcher { conn, path, kind: PosKind::Int }
    }

    /// Add a mod row whose `steamId` is `id`.
    pub(crate) fn mod_row(&self, id: &str) {
        self.conn
            .execute(
                "INSERT INTO mods (id, name, displayName, steamId) VALUES (?1, ?1, ?1, ?1)",
                [id],
            )
            .expect("insert mod");
    }

    /// Add a playset.
    pub(crate) fn playset(&self, id: &str, active: bool, removed: bool) {
        self.conn
            .execute(
                "INSERT INTO playsets (id, name, isActive, isRemoved) VALUES (?1, ?1, ?2, ?3)",
                rusqlite::params![id, active, removed],
            )
            .expect("insert playset");
    }

    /// Put `(mod id, enabled)` into `playset` at positions in the given order.
    pub(crate) fn fill(&self, playset: &str, order: &[(&str, bool)]) {
        for (n, (id, on)) in order.iter().enumerate() {
            let sql = format!(
                "INSERT INTO playsets_mods (playsetId, modId, position, enabled) VALUES (?1, ?2, {}, ?3)",
                pos_sql(self.kind, n)
            );
            self.conn.execute(&sql, rusqlite::params![playset, id, on]).expect("insert entry");
        }
    }

    /// One active playset `p1` over these mods, in this order, all enabled.
    pub(crate) fn seed(&self, ids: &[&str]) {
        for id in ids {
            self.mod_row(id);
        }
        self.playset("p1", true, false);
        let order: Vec<(&str, bool)> = ids.iter().map(|i| (*i, true)).collect();
        self.fill("p1", &order);
    }

    /// Fold the WAL into the main file and empty it.
    pub(crate) fn checkpoint(&self) {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").expect("checkpoint");
    }

    /// Reorder `p1` to `ids` with one commit that stays in the `-wal`.
    pub(crate) fn reorder(&self, ids: &[&str]) {
        self.conn.execute_batch("BEGIN").expect("begin");
        self.conn.execute("DELETE FROM playsets_mods WHERE playsetId = 'p1'", []).expect("clear");
        let order: Vec<(&str, bool)> = ids.iter().map(|i| (*i, true)).collect();
        self.fill("p1", &order);
        self.conn.execute_batch("COMMIT").expect("commit");
    }
}
