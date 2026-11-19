//! `clausewitz`: PDX script, through the extractor the path's class names.

use smd_core::{FileKind, Span};

use crate::extract::{DepthKey, EventExtractor, Out};
use crate::registry::{FileParser, ParseInput, ParseOutput};
use crate::script::{Diags, ScriptSink, codes, parse};
use crate::text::{Encoding, detect};

/// The PDX script parser.
pub struct ClausewitzScriptParser;

/// Syntax checking only, for a class whose extractor emits nothing.
struct Discard<'a>(Out<'a>);

impl ScriptSink for Discard<'_> {
    fn diags(&mut self) -> &mut Diags {
        &mut self.0.diags
    }
}

impl FileParser for ClausewitzScriptParser {
    fn id(&self) -> &'static str {
        "clausewitz"
    }

    fn version(&self) -> u32 {
        1
    }

    fn accepts(&self, kind: FileKind) -> bool {
        kind == FileKind::Script
    }

    fn parse(&self, input: &ParseInput<'_>) -> ParseOutput {
        let (start, enc) = detect(input.bytes);
        let mut diags = Diags::default();
        if enc == Encoding::Windows1252 {
            let detail = "not UTF-8; decoded as Windows-1252".to_owned();
            diags.push(codes::ENCODING, Span::default(), detail, false);
        }
        let out = Out { bytes: input.bytes, enc, facts: Vec::new(), diags };
        let (bytes, cancelled, class) = (input.bytes, input.cancelled, input.class);
        let out = match (class.extractor, class.domain) {
            ("depth_key", Some(domain)) => {
                let mut sink = DepthKey::new(out, class, domain);
                parse(bytes, start, &mut sink, cancelled);
                sink.out
            }
            ("pdx_event", _) => {
                let mut sink = EventExtractor::new(out);
                parse(bytes, start, &mut sink, cancelled);
                sink.out
            }
            _ => {
                let mut sink = Discard(out);
                parse(bytes, start, &mut sink, cancelled);
                sink.0
            }
        };
        ParseOutput::from_parts(out.facts, out.diags)
    }
}
