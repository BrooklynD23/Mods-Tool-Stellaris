//! Workshop content directories, each read from its own `descriptor.mod`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use smd_core::{DescriptorSource, SourceRole};
use smd_fs::ReadOnlyRoot;

use super::candidate::{Raw, Sink, is_invalid, open_failed, read_descriptor, unread};
use super::listing::{self, Entry, Kind};
use super::{ContentState, ModOrigin, UnreadReason};
use crate::codes::{self, IngestDiagnostic};

/// Workshop content directories not already claimed by a launcher `*.mod`, each read from
/// its own `descriptor.mod`. A directory with no readable `descriptor.mod` is still a mod
/// on disk: it is inventoried under its directory name, content unread, with a diagnostic.
pub(super) fn workshop_mods(
    workshop: &Path,
    claimed: &[PathBuf],
    same: &dyn Fn(&Path, &Path) -> bool,
    sink: &mut Sink,
) -> Vec<Raw> {
    let root = match ReadOnlyRoot::open(workshop) {
        Ok(r) => r,
        Err(e) => {
            open_failed(workshop, super::KEY_WORKSHOP, &e, sink);
            return Vec::new();
        }
    };
    let top = listing::list(root.path(), super::KEY_WORKSHOP, |_| true, &mut sink.diags);
    sink.incomplete |= top.incomplete;
    let mut out = Vec::new();
    for e in top.entries.iter().filter(|e| e.kind == Kind::Dir) {
        let dir_name = e.text();
        let here = root.path().join(&e.name);
        if claimed.iter().any(|c| same(c, &here)) {
            continue;
        }
        out.push(one_workshop_dir(&root, e, &dir_name, sink));
    }
    out
}

pub(super) fn is_descriptor_name(n: &str) -> bool {
    n.eq_ignore_ascii_case("descriptor.mod")
}

/// The mod whose `descriptor.mod` sits in the directory `rel` below `root` (`rel` empty:
/// `root` itself). `None` when there is no `descriptor.mod` there; a descriptor that is
/// over a cap or unreadable is still a `Some`, content unread, with its diagnostic.
pub(super) fn dir_mod(
    root: &ReadOnlyRoot,
    rel: &[OsString],
    dir_name: &str,
    label: &str,
    sink: &mut Sink,
) -> Option<Raw> {
    let dir = rel.iter().fold(root.path().to_path_buf(), |p, c| p.join(c));
    let inner = listing::list(&dir, label, is_descriptor_name, &mut sink.diags);
    sink.incomplete |= inner.incomplete;
    let file =
        inner.entries.iter().find(|e| e.kind == Kind::File && is_descriptor_name(&e.text()))?;
    let mut raw = unread(dir_name, None, UnreadReason::DescriptorUnreadable);
    let mut os_path = rel.to_vec();
    os_path.push(file.name.clone());
    let (desc, dd) = match read_descriptor(root, &os_path, file.len) {
        Ok(x) => x,
        Err(e) => {
            sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, dir_name, e));
            return Some(raw);
        }
    };
    raw.diagnostics = dd;
    if is_invalid(&raw.diagnostics) {
        raw.content = ContentState::Unread(UnreadReason::DescriptorInvalid);
    } else {
        raw.descriptor = desc;
        raw.content_root = Some(dir);
        raw.content = ContentState::Readable;
    }
    Some(raw)
}

fn one_workshop_dir(root: &ReadOnlyRoot, dir: &Entry, dir_name: &str, sink: &mut Sink) -> Raw {
    let label = format!("{}/{dir_name}", super::KEY_WORKSHOP);
    let found = dir_mod(root, std::slice::from_ref(&dir.name), dir_name, &label, sink);
    let mut raw = found.unwrap_or_else(|| {
        let why = "has no readable descriptor.mod; inventoried by directory name, content unread";
        sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, dir_name, why));
        unread(dir_name, None, UnreadReason::DescriptorUnreadable)
    });
    raw.role = SourceRole::Workshop;
    raw.origin = ModOrigin::WorkshopDir;
    raw.source_path = Some(root.path().join(&dir.name));
    if raw.content == ContentState::Readable {
        raw.descriptor.source = DescriptorSource::Workshop;
    }
    raw
}
