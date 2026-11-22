//! `libraryfolders.vdf` and `appmanifest_*.acf`: never panic, whatever the bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = smd_fs::parse_library_folders(text);
        let _ = smd_fs::parse_install_dir(text);
    }
});
