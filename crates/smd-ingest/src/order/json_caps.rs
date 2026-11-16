//! Caps for launcher JSON (`dlc_load.json` and kin), enforced before and after parsing.
//!
//! `serde_json`'s recursion limit is fixed at 128 and cannot be set to the 64 the design
//! calls for, so depth is checked by a byte-level pre-scan that never allocates.

/// Largest launcher JSON file read.
pub const FILE_MAX: u64 = 16 * 1024 * 1024;
/// Deepest nesting accepted.
pub const DEPTH_MAX: usize = 64;
/// Most array elements and object members, in total.
pub const ENTRIES_MAX: usize = 20_000;

/// Reject `bytes` if brackets nest deeper than [`DEPTH_MAX`]. Strings are skipped, so a
/// bracket inside one does not count; malformed input is left for the parser to refuse.
///
/// # Errors
/// A description of the cap that tripped.
pub fn check_depth(bytes: &[u8]) -> Result<(), String> {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for &b in bytes {
        if in_string {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > DEPTH_MAX {
                    return Err(format!("nesting is deeper than the {DEPTH_MAX}-level cap"));
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Reject a parsed value with more than [`ENTRIES_MAX`] elements and members in total.
///
/// # Errors
/// A description of the cap that tripped.
pub fn check_entries(root: &serde_json::Value) -> Result<(), String> {
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::Array(a) => {
                count += a.len();
                stack.extend(a);
            }
            serde_json::Value::Object(o) => {
                count += o.len();
                stack.extend(o.values());
            }
            _ => {}
        }
        if count > ENTRIES_MAX {
            return Err(format!("more than {ENTRIES_MAX} entries"));
        }
    }
    Ok(())
}
