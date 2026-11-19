//! `descriptor.mod` / `*.mod`: never panic, always return within the caps.
#![no_main]

use libfuzzer_sys::fuzz_target;
use smd_core::{DescriptorReader, PathId, Site, SourceId, Span};
use smd_ingest::KeyValuesReader;

fuzz_target!(|data: &[u8]| {
    let site = Site { source: SourceId(1), path: PathId(0), span: Span::default() };
    let _ = KeyValuesReader.read(data, site);
});
