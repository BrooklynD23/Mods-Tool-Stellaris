//! The launcher-written `dlc_load.json`: caps hold and nothing panics.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut diags = Vec::new();
    let _ = smd_ingest::parse_dlc_load_json(data, &mut diags);
});
