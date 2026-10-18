//! The JSON renderer: the view model, serialised by `serde_json`. Strings are carried
//! exactly; escaping is serde's plus `escape::json`, and the output is always valid UTF-8.

use crate::model::{RenderOptions, ReportModel};
use crate::{Renderer, build};

/// Pretty-printed JSON with a trailing newline.
#[derive(Copy, Clone, Debug, Default)]
pub struct Json;

impl Renderer for Json {
    fn render(&self, model: &ReportModel, opts: &RenderOptions) -> String {
        let v = build::view(model, opts);
        encode(&v)
    }
}

/// The diagnostics stream as a JSON array. Used by [`crate::render_diagnostics`].
pub(crate) fn diagnostics_stream(rows: &[crate::view::DiagView]) -> String {
    encode(&rows)
}

fn encode<T: serde::Serialize>(value: &T) -> String {
    // The view holds only strings, integers, and options, none of which can fail to
    // serialise; the fallback keeps the function total without a panic path.
    let mut s = serde_json::to_string_pretty(value)
        .unwrap_or_else(|e| format!("{{\"error\":\"report serialisation failed: {e}\"}}"));
    s = crate::escape::json::harden(&s);
    s.push('\n');
    s
}
