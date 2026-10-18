//! Workshop content directories, each read from its own `descriptor.mod`.

use std::path::{Path, PathBuf};

use smd_core::{Descriptor, DescriptorSource, SourceRole};
use smd_fs::ReadOnlyRoot;

use super::candidate::{Raw, Sink, is_invalid, open_failed, read_descriptor, unread};
use super::listing::{self, Entry, Kind};
use super::{ContentState, UnreadReason};
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

fn is_descriptor_name(n: &str) -> bool {
    n.eq_ignore_ascii_case("descriptor.mod")
}

fn one_workshop_dir(root: &ReadOnlyRoot, dir: &Entry, dir_name: &str, sink: &mut Sink) -> Raw {
    let label = format!("{}/{dir_name}", super::KEY_WORKSHOP);
    let inner =
        listing::list(&root.path().join(&dir.name), &label, is_descriptor_name, &mut sink.diags);
    sink.incomplete |= inner.incomplete;
    let mut raw = unread(dir_name, None, UnreadReason::DescriptorUnreadable);
    raw.role = SourceRole::Workshop;
    let Some(file) =
        inner.entries.iter().find(|e| e.kind == Kind::File && is_descriptor_name(&e.text()))
    else {
        let why = "has no readable descriptor.mod; inventoried by directory name, content unread";
        sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, dir_name, why));
        return raw;
    };
    let os_path = [dir.name.clone(), file.name.clone()];
    let (desc, dd) = match read_descriptor(root, &os_path, file.len) {
        Ok(x) => x,
        Err(e) => {
            sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, dir_name, e));
            return raw;
        }
    };
    raw.diagnostics = dd;
    if is_invalid(&raw.diagnostics) {
        raw.content = ContentState::Unread(UnreadReason::DescriptorInvalid);
    } else {
        raw.descriptor = Descriptor { source: DescriptorSource::Workshop, ..desc };
        raw.content_root = Some(root.path().join(&dir.name));
        raw.content = ContentState::Readable;
    }
    raw
}
