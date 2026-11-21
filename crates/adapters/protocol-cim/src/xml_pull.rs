// FIM-F06.01 — the hand-written minimal RDF/XML PULL parser for the CGMES
// profile subset (docs/releases/final-implementation-before-publication.md
// §5 feature F-06; ledger docs/releases/implementation_ledger/ledger_F06.md;
// decision record 44_p14_12_cim_cgmes_import.md).
//
// **Implemented under FIM-F06, WRITTEN-UNRUN: this module has never been
// compiled or executed. Interop with a real DSO CGMES profile is UNPROVEN
// and remains a HUMAN LAB GATE** (crate conventions §7.3; nothing here is
// a conformance result).
//
// THE DEPENDENCY RULE (FIM standing decision 1): NO XML crate enters the
// resolution graph — quick-xml/roxmltree/xml5ever are not in the committed
// Cargo.lock — so the parser below is a HAND-WRITTEN byte-level pull
// scanner over the CGMES profile SUBSET of RDF/XML, pinned by the
// round-trip and malformed-refusal test vectors in this module. It is a
// subset scanner, not an RDF/XML processor:
//
// - IN: elements, attributes, namespace prefixes bound by `xmlns` /
//   `xmlns:prefix` declarations (an undeclared prefix is a NAMED refusal),
//   the five XML named entities plus decimal/hexadecimal character
//   references, comments, processing instructions, whitespace text nodes.
// - OUT (each a NAMED refusal with this honesty header, never a silent
//   skip): CDATA sections, DTDs (any `<!DOCTYPE` — which also rules out
//   custom entity definitions and external entity resolution: none exists
//   here), XInclude elements, unknown entity references.
// - Well-formedness refusals, each named: an unclosed element at EOF, a
//   mismatched close tag, stray non-whitespace text outside the root, a
//   duplicate attribute, markup after the root element, empty input, a
//   raw '<' inside an attribute value, a malformed qualified name.
// - The bounded-input guard: input above [`MAX_XML_INPUT_BYTES`] is
//   refused before any scan ([`XmlParseError::InputTooLarge`]).
//
// Namespaces follow XML Namespaces as far as the subset needs them: a
// prefix is resolved through the xmlns declarations of the enclosing
// elements (a declaration applies to its own element too); an unprefixed
// ELEMENT name resolves through the default namespace (an undeclared
// default = no namespace); an unprefixed ATTRIBUTE name is in NO namespace
// (the default namespace never applies to attributes). Duplicate
// attributes are detected on the EXPANDED name (namespace + local), so two
// prefixes bound to the same URI carrying the same local name is still a
// duplicate.

/// The bounded-input guard (FIM-F06.01): a document above this size is
/// refused before any scan. A real DSO exchange profile stays far below
/// this; the guard bounds the parse, never the deployment (a composition
/// root that legitimately needs more re-declares the limit through
/// [`XmlPullParser::with_max_bytes`] at its own reviewed boundary).
pub const MAX_XML_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// The RDF namespace URI the profile root must carry (the one namespace
/// URI this parser hard-declares; the `cim:` profile namespace URIs are
/// NOT declared here — the consumer classifies by element name, and the
/// exact CGMES namespace spelling stays an implementation-time check
/// against the CGMES profile text [S-17]).
pub const RDF_NAMESPACE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// The XInclude namespace: an element in this namespace is a NAMED refusal
/// (no XInclude resolution exists in this subset — the honesty header).
pub const XINCLUDE_NAMESPACE: &str = "http://www.w3.org/2001/XInclude";

/// Errors returned by the pull parser. Every refusal is NAMED (the
/// FIM-F06.01 subtask list); the `byte` field is the input byte offset the
/// refusal was raised at — carried for the human gate, never formatted
/// raw into operator-facing messages (the P1.08 rule lives one layer up,
/// in the profile-source seam).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlParseError {
    /// The input exceeds [`MAX_XML_INPUT_BYTES`] (or the constructor's
    /// explicit limit) — refused before any scan.
    InputTooLarge {
        max_bytes: usize,
        observed_bytes: usize,
    },
    /// The input is not valid UTF-8 (this subset parses UTF-8 text only).
    InvalidUtf8 {
        byte: usize,
    },
    /// A prefixed name uses a prefix no enclosing `xmlns:prefix`
    /// declaration binds.
    UndeclaredPrefix {
        prefix: String,
        byte: usize,
    },
    /// An entity reference outside the five XML named entities and the
    /// decimal/hexadecimal character references (custom entities would
    /// need a DTD — refused above).
    UnknownEntity {
        name: String,
        byte: usize,
    },
    /// The same expanded attribute name (namespace + local) appears twice
    /// on one start tag — also covers a twice-bound `xmlns` prefix.
    DuplicateAttribute {
        name: String,
        byte: usize,
    },
    /// A `<![CDATA[ ... ]]>` section: outside the declared subset, a named
    /// refusal (never silently skipped).
    CdataUnsupported {
        byte: usize,
    },
    /// A `<!DOCTYPE ...>` declaration: outside the declared subset — no
    /// DTD processing, no custom entities, no external entity resolution.
    DtdUnsupported {
        byte: usize,
    },
    /// An element in the XInclude namespace: outside the declared subset.
    XIncludeUnsupported {
        byte: usize,
    },
    /// The input ended while an element was still open.
    UnclosedElement {
        name: String,
        open_byte: usize,
    },
    /// A close tag does not match the innermost open element.
    MismatchedCloseTag {
        expected: String,
        found: String,
        byte: usize,
    },
    /// Non-whitespace text outside the root element (before it or after
    /// it) — a well-formedness refusal (stray text is never skipped).
    TextOutsideRoot {
        byte: usize,
    },
    /// Any other well-formedness refusal of the subset (empty input, no
    /// root element, markup after the root, unterminated markup, a raw '<'
    /// in an attribute value, a malformed qualified name, an invalid
    /// character reference).
    Malformed {
        reason: String,
        byte: usize,
    },
}

impl std::fmt::Display for XmlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlParseError::InputTooLarge {
                max_bytes,
                observed_bytes,
            } => write!(
                f,
                "the input is {observed_bytes} bytes, above the bounded-input guard \
                 ({max_bytes} bytes) — refused before any scan"
            ),
            XmlParseError::InvalidUtf8 { byte } => {
                write!(f, "the input is not valid UTF-8 (first invalid byte at {byte})")
            }
            XmlParseError::UndeclaredPrefix { prefix, byte } => write!(
                f,
                "the prefix '{prefix}' is not bound by any enclosing xmlns declaration \
                 (byte {byte})"
            ),
            XmlParseError::UnknownEntity { name, byte } => write!(
                f,
                "the entity reference '&{name};' is not one of the five XML named \
                 entities or a character reference (byte {byte})"
            ),
            XmlParseError::DuplicateAttribute { name, byte } => write!(
                f,
                "the attribute '{name}' appears twice on one start tag (byte {byte})"
            ),
            XmlParseError::CdataUnsupported { byte } => write!(
                f,
                "a CDATA section is outside the CGMES profile subset this parser \
                 declares — refused, never skipped (byte {byte})"
            ),
            XmlParseError::DtdUnsupported { byte } => write!(
                f,
                "a DTD declaration is outside the CGMES profile subset this parser \
                 declares — no DTD processing, no custom entities, no external entity \
                 resolution (byte {byte})"
            ),
            XmlParseError::XIncludeUnsupported { byte } => write!(
                f,
                "an XInclude element is outside the CGMES profile subset this parser \
                 declares — refused, never resolved (byte {byte})"
            ),
            XmlParseError::UnclosedElement { name, open_byte } => write!(
                f,
                "the element '{name}' (opened at byte {open_byte}) is never closed"
            ),
            XmlParseError::MismatchedCloseTag {
                expected,
                found,
                byte,
            } => write!(
                f,
                "the close tag </{found}> does not match the open element '{expected}' \
                 (byte {byte})"
            ),
            XmlParseError::TextOutsideRoot { byte } => write!(
                f,
                "non-whitespace text appears outside the root element (byte {byte})"
            ),
            XmlParseError::Malformed { reason, byte } => {
                write!(f, "the document is not well-formed for this subset: {reason} (byte {byte})")
            }
        }
    }
}

impl std::error::Error for XmlParseError {}

/// A resolved XML name: the namespace URI the prefix (or default
/// namespace) resolves to, plus the local name. The raw prefixed spelling
/// is NOT carried here — the parser refuses undeclared prefixes instead of
/// passing them through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlQName {
    /// The resolved namespace URI ("" = no namespace).
    pub namespace: String,
    /// The local name (after the ':').
    pub local: String,
}

/// A start element: the resolved name and the resolved, entity-decoded
/// attributes (xmlns declarations are consumed into the scope and are NOT
/// carried as attributes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlStartElement {
    pub name: XmlQName,
    pub attributes: Vec<(XmlQName, String)>,
}

impl XmlStartElement {
    /// The value of the attribute in the given namespace with the given
    /// local name, if present.
    pub fn attribute(&self, namespace: &str, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(name, _)| name.namespace == namespace && name.local == local)
            .map(|(_, value)| value.as_str())
    }
}

/// The pull events: start element (with attributes), end element, and
/// non-whitespace text (entity-decoded). Whitespace-only text nodes are
/// skipped by the parser itself — they carry no profile content (the
/// declared subset rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlEvent {
    StartElement(XmlStartElement),
    EndElement(XmlQName),
    Text(String),
}

/// One open element on the parser's stack: its raw qualified spelling
/// (close-tag matching is on the RAW spelling), its resolved name, the
/// byte it opened at, and the namespace declarations it introduced.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenElement {
    raw: String,
    name: XmlQName,
    open_byte: usize,
    namespace_frame: Vec<(String, String)>,
}

/// The hand-written minimal RDF/XML pull parser (FIM-F06.01). Constructed
/// over the full input bytes — the bounded-input guard runs at
/// construction, before any scan — and driven event by event through
/// [`XmlPullParser::next_event`]. No I/O, no allocation-heavy
/// intermediate tree: the consumer (the profile source) streams the
/// events into the CIM object vocabulary.
#[derive(Debug, Clone)]
pub struct XmlPullParser<'a> {
    input: &'a str,
    pos: usize,
    root_seen: bool,
    open: Vec<OpenElement>,
    pending_end: Option<XmlQName>,
}

impl<'a> XmlPullParser<'a> {
    /// Constructs the parser with the module's bounded-input guard.
    pub fn new(input: &'a [u8]) -> Result<Self, XmlParseError> {
        Self::with_max_bytes(input, MAX_XML_INPUT_BYTES)
    }

    /// Constructs the parser with an explicit bounded-input limit (the
    /// pinned-test seam for [`XmlParseError::InputTooLarge`]).
    pub fn with_max_bytes(input: &'a [u8], max_bytes: usize) -> Result<Self, XmlParseError> {
        if input.len() > max_bytes {
            return Err(XmlParseError::InputTooLarge {
                max_bytes,
                observed_bytes: input.len(),
            });
        }
        let text = std::str::from_utf8(input).map_err(|error| XmlParseError::InvalidUtf8 {
            byte: error.valid_up_to(),
        })?;
        Ok(Self {
            input: text,
            pos: 0,
            root_seen: false,
            open: Vec::new(),
            pending_end: None,
        })
    }

    /// Produces the next event, or `None` after the root element closed
    /// and the trailing whitespace was consumed. Every well-formedness
    /// refusal of the module header surfaces here as a named error; a
    /// partial document is never a success.
    pub fn next_event(&mut self) -> Result<Option<XmlEvent>, XmlParseError> {
        // A self-closing start tag delivers its matching end element on
        // the following call (the pending-end slot; the scope frame pops
        // with it).
        if let Some(name) = self.pending_end.take() {
            self.open.pop();
            return Ok(Some(XmlEvent::EndElement(name)));
        }
        loop {
            if self.pos >= self.input.len() {
                if let Some(open) = self.open.last() {
                    return Err(XmlParseError::UnclosedElement {
                        name: open.raw.clone(),
                        open_byte: open.open_byte,
                    });
                }
                if !self.root_seen {
                    return Err(XmlParseError::Malformed {
                        reason: "the document contains no root element".to_string(),
                        byte: self.pos,
                    });
                }
                return Ok(None);
            }
            if self.input.as_bytes()[self.pos] == b'<' {
                match self.scan_markup()? {
                    Some(event) => return Ok(Some(event)),
                    None => continue,
                }
            }
            match self.scan_text()? {
                Some(event) => return Ok(Some(event)),
                None => continue,
            }
        }
    }

    /// Scans one markup construct starting at '<'. Comments and processing
    /// instructions return `None` (keep scanning); element tags return the
    /// event.
    fn scan_markup(&mut self) -> Result<Option<XmlEvent>, XmlParseError> {
        let start = self.pos;
        if self.root_seen && self.open.is_empty() {
            return Err(XmlParseError::Malformed {
                reason: "markup appears after the root element has closed".to_string(),
                byte: start,
            });
        }
        let text: &'a str = self.input;
        let tail = &text[start..];
        if tail.starts_with("<!--") {
            let Some(end) = tail.find("-->") else {
                return Err(XmlParseError::Malformed {
                    reason: "a comment is never closed by '-->'".to_string(),
                    byte: start,
                });
            };
            self.pos = start + end + 3;
            return Ok(None);
        }
        if tail.starts_with("<![CDATA[") {
            return Err(XmlParseError::CdataUnsupported { byte: start });
        }
        if tail.starts_with("<!DOCTYPE") {
            return Err(XmlParseError::DtdUnsupported { byte: start });
        }
        if tail.starts_with("<!") {
            return Err(XmlParseError::Malformed {
                reason: "an unknown '<!' markup declaration (only comments are supported \
                         in this subset)"
                    .to_string(),
                byte: start,
            });
        }
        if tail.starts_with("<?") {
            let Some(end) = tail.find("?>") else {
                return Err(XmlParseError::Malformed {
                    reason: "a processing instruction is never closed by '?>'".to_string(),
                    byte: start,
                });
            };
            self.pos = start + end + 2;
            return Ok(None);
        }
        if tail.starts_with("</") {
            return self.scan_end_tag();
        }
        self.scan_start_tag()
    }

    /// Scans a close tag; the RAW spelling must match the innermost open
    /// element (the well-formedness rule the parser enforces itself).
    fn scan_end_tag(&mut self) -> Result<Option<XmlEvent>, XmlParseError> {
        let start = self.pos;
        self.pos += 2; // "</"
        let raw_name = self.scan_name()?;
        self.skip_ws();
        self.expect_byte(b'>')?;
        let Some(open) = self.open.last() else {
            return Err(XmlParseError::Malformed {
                reason: format!("the close tag </{raw_name}> has no matching open element"),
                byte: start,
            });
        };
        if open.raw != raw_name {
            return Err(XmlParseError::MismatchedCloseTag {
                expected: open.raw.clone(),
                found: raw_name,
                byte: start,
            });
        }
        let resolved = open.name.clone();
        self.open.pop();
        Ok(Some(XmlEvent::EndElement(resolved)))
    }

    /// Scans a start tag: the name, the attributes (namespace declarations
    /// first), the resolved names, and — for a self-closing tag — the
    /// pending end element.
    fn scan_start_tag(&mut self) -> Result<Option<XmlEvent>, XmlParseError> {
        let start = self.pos;
        // Copy the input reference out of `self` (a &'a str is Copy): the
        // scan helpers below take &mut self, so no borrow of the field may
        // stay alive across those calls.
        let text: &'a str = self.input;
        self.pos += 1; // '<'
        let raw_name = self.scan_name()?;
        let mut raw_attributes: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;
        loop {
            self.skip_ws();
            let bytes = text.as_bytes();
            if self.pos >= bytes.len() {
                return Err(XmlParseError::Malformed {
                    reason: format!("the start tag <{raw_name}> is never terminated"),
                    byte: start,
                });
            }
            let byte = bytes[self.pos];
            match byte {
                b'>' => {
                    self.pos += 1;
                    break;
                }
                b'/' => {
                    self.pos += 1;
                    self.expect_byte(b'>')?;
                    self_closing = true;
                    break;
                }
                _ => {
                    let raw_attribute = self.scan_name()?;
                    self.expect_byte(b'=')?;
                    let value = self.scan_attribute_value()?;
                    raw_attributes.push((raw_attribute, value));
                }
            }
        }
        // Namespace declarations first (XML Namespaces: a declaration
        // applies to its own element). A twice-bound prefix is a duplicate
        // attribute.
        let mut frame: Vec<(String, String)> = Vec::new();
        for (raw, value) in &raw_attributes {
            if raw == "xmlns" {
                if frame.iter().any(|(bound, _)| bound.is_empty()) {
                    return Err(XmlParseError::DuplicateAttribute {
                        name: "xmlns".to_string(),
                        byte: start,
                    });
                }
                frame.push((String::new(), value.clone()));
            } else if let Some(prefix) = raw.strip_prefix("xmlns:") {
                if prefix.is_empty() {
                    return Err(XmlParseError::Malformed {
                        reason: "a namespace declaration cannot bind an empty prefix"
                            .to_string(),
                        byte: start,
                    });
                }
                if frame.iter().any(|(bound, _)| bound == prefix) {
                    return Err(XmlParseError::DuplicateAttribute {
                        name: raw.clone(),
                        byte: start,
                    });
                }
                frame.push((prefix.to_string(), value.clone()));
            }
        }
        self.open.push(OpenElement {
            raw: raw_name.clone(),
            name: XmlQName {
                namespace: String::new(),
                local: String::new(),
            },
            open_byte: start,
            namespace_frame: frame,
        });
        let (prefix, local) = split_qname(&raw_name)?;
        let namespace = self.resolve_prefix(&prefix, start)?;
        let resolved_name = XmlQName { namespace, local };
        if resolved_name.namespace == XINCLUDE_NAMESPACE {
            return Err(XmlParseError::XIncludeUnsupported { byte: start });
        }
        // Resolve the remaining attribute names. Duplicate detection is on
        // the EXPANDED name: two prefixes bound to the same URI carrying
        // the same local name is still a duplicate.
        let mut attributes: Vec<(XmlQName, String)> = Vec::new();
        let mut seen: Vec<(String, String)> = Vec::new();
        for (raw, value) in &raw_attributes {
            if raw == "xmlns" || raw.starts_with("xmlns:") {
                continue; // consumed into the scope frame above
            }
            let (attribute_prefix, attribute_local) = split_qname(raw)?;
            let attribute_namespace = if attribute_prefix.is_empty() {
                // XML Namespaces: an unprefixed attribute is in NO
                // namespace — the default namespace does not apply to
                // attributes.
                String::new()
            } else {
                self.resolve_prefix(&attribute_prefix, start)?
            };
            let expanded = (attribute_namespace.clone(), attribute_local.clone());
            if seen.contains(&expanded) {
                return Err(XmlParseError::DuplicateAttribute {
                    name: raw.clone(),
                    byte: start,
                });
            }
            seen.push(expanded);
            attributes.push((
                XmlQName {
                    namespace: attribute_namespace,
                    local: attribute_local,
                },
                value.clone(),
            ));
        }
        if let Some(open) = self.open.last_mut() {
            open.name = resolved_name.clone();
        }
        if self.open.len() == 1 {
            self.root_seen = true;
        }
        if self_closing {
            self.pending_end = Some(resolved_name.clone());
        }
        Ok(Some(XmlEvent::StartElement(XmlStartElement {
            name: resolved_name,
            attributes,
        })))
    }

    /// Scans one quoted attribute value with entity decoding; a raw '<'
    /// inside an attribute value is refused (the XML well-formedness rule).
    fn scan_attribute_value(&mut self) -> Result<String, XmlParseError> {
        let start = self.pos;
        let text: &'a str = self.input;
        let bytes = text.as_bytes();
        if self.pos >= bytes.len() || (bytes[self.pos] != b'"' && bytes[self.pos] != b'\'') {
            return Err(XmlParseError::Malformed {
                reason: "an attribute value must be quoted".to_string(),
                byte: start,
            });
        }
        let quote = bytes[self.pos];
        self.pos += 1;
        let value_start = self.pos;
        let Some(end) = text[value_start..].find(quote as char) else {
            return Err(XmlParseError::Malformed {
                reason: "an attribute value is never closed by its quote".to_string(),
                byte: start,
            });
        };
        let raw = &text[value_start..value_start + end];
        self.pos = value_start + end + 1;
        if raw.contains('<') {
            return Err(XmlParseError::Malformed {
                reason: "a raw '<' is not allowed inside an attribute value".to_string(),
                byte: value_start,
            });
        }
        decode_entities(raw, value_start)
    }

    /// Scans one text run up to the next '<' (or EOF), decodes its
    /// entities, and classifies it: whitespace-only runs return `None`
    /// (skipped); non-whitespace runs outside the root are a named
    /// refusal; the rest is a Text event.
    fn scan_text(&mut self) -> Result<Option<XmlEvent>, XmlParseError> {
        let start = self.pos;
        let text: &'a str = self.input;
        let end = text[start..]
            .find('<')
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        let raw = &text[start..end];
        self.pos = end;
        let decoded = decode_entities(raw, start)?;
        let is_xml_whitespace = decoded
            .chars()
            .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
        if is_xml_whitespace {
            return Ok(None);
        }
        if self.open.is_empty() {
            return Err(XmlParseError::TextOutsideRoot { byte: start });
        }
        Ok(Some(XmlEvent::Text(decoded)))
    }

    /// Resolves a prefix through the open elements' namespace frames
    /// (innermost first). An unprefixed name with no declared default
    /// namespace resolves to no namespace; a prefixed name without any
    /// binding is the named refusal.
    fn resolve_prefix(&self, prefix: &str, byte: usize) -> Result<String, XmlParseError> {
        for open in self.open.iter().rev() {
            for (bound, uri) in &open.namespace_frame {
                if bound == prefix {
                    return Ok(uri.clone());
                }
            }
        }
        if prefix.is_empty() {
            Ok(String::new())
        } else {
            Err(XmlParseError::UndeclaredPrefix {
                prefix: prefix.to_string(),
                byte,
            })
        }
    }

    /// Scans one NCName-style qualified name (at most one ':' — the prefix/
    /// local split happens in [`split_qname`]).
    fn scan_name(&mut self) -> Result<String, XmlParseError> {
        let start = self.pos;
        let text: &'a str = self.input;
        let mut chars = text[start..].chars();
        let Some(first) = chars.next() else {
            return Err(XmlParseError::Malformed {
                reason: "a name was expected but the document ended".to_string(),
                byte: start,
            });
        };
        if !(first.is_alphabetic() || first == '_') {
            return Err(XmlParseError::Malformed {
                reason: format!("a name must start with a letter or '_' (found '{first}')"),
                byte: start,
            });
        }
        let mut end = start + first.len_utf8();
        let mut colons = 0usize;
        for c in chars {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                end += c.len_utf8();
            } else if c == ':' {
                colons += 1;
                end += 1;
            } else {
                break;
            }
        }
        let name = &text[start..end];
        if colons > 1 {
            return Err(XmlParseError::Malformed {
                reason: "a qualified name carries more than one ':'".to_string(),
                byte: start,
            });
        }
        if name.contains(':') && name.split_once(':').is_none_or(|(prefix, local)| {
            prefix.is_empty() || local.is_empty()
        }) {
            return Err(XmlParseError::Malformed {
                reason: format!("the qualified name '{name}' has an empty prefix or local part"),
                byte: start,
            });
        }
        self.pos = end;
        Ok(name.to_string())
    }

    fn skip_ws(&mut self) {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && matches!(bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n') {
            self.pos += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), XmlParseError> {
        let bytes = self.input.as_bytes();
        if self.pos < bytes.len() && bytes[self.pos] == expected {
            self.pos += 1;
            Ok(())
        } else {
            Err(XmlParseError::Malformed {
                reason: format!("expected '{}' at this position", expected as char),
                byte: self.pos,
            })
        }
    }
}

/// Splits a raw qualified name into (prefix, local); a name without ':'
/// carries the empty prefix. An empty prefix or local part is refused.
fn split_qname(raw: &str) -> Result<(String, String), XmlParseError> {
    match raw.split_once(':') {
        Some((prefix, local)) if !prefix.is_empty() && !local.is_empty() => {
            Ok((prefix.to_string(), local.to_string()))
        }
        Some(_) => Err(XmlParseError::Malformed {
            reason: format!("the qualified name '{raw}' has an empty prefix or local part"),
            byte: 0,
        }),
        None => Ok((String::new(), raw.to_string())),
    }
}

/// Decodes the entity references of one raw text/attribute-value run: the
/// five XML named entities plus decimal (`&#NNN;`) and hexadecimal
/// (`&#xHH;`) character references. Anything else (including an
/// unterminated reference and a control character XML 1.0 forbids) is a
/// NAMED refusal — never silently copied through.
fn decode_entities(raw: &str, byte_base: usize) -> Result<String, XmlParseError> {
    if !raw.contains('&') {
        return Ok(raw.to_string());
    }
    let mut decoded = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < raw.len() {
        if bytes[i] != b'&' {
            let ch = raw[i..]
                .chars()
                .next()
                .expect("a valid &str always yields a char at a char boundary");
            decoded.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let byte = byte_base + i;
        let rest = &raw[i + 1..];
        let Some(semi) = rest.find(';') else {
            return Err(XmlParseError::Malformed {
                reason: "an entity reference is never terminated by ';'".to_string(),
                byte,
            });
        };
        let name = &rest[..semi];
        match name {
            "amp" => decoded.push('&'),
            "lt" => decoded.push('<'),
            "gt" => decoded.push('>'),
            "quot" => decoded.push('"'),
            "apos" => decoded.push('\''),
            "" => {
                return Err(XmlParseError::Malformed {
                    reason: "an empty entity reference ';&'".to_string(),
                    byte,
                });
            }
            _ => {
                let code = if let Some(hex) = name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).map_err(|_| XmlParseError::UnknownEntity {
                        name: name.to_string(),
                        byte,
                    })?
                } else if let Some(decimal) = name.strip_prefix('#') {
                    decimal.parse::<u32>().map_err(|_| XmlParseError::UnknownEntity {
                        name: name.to_string(),
                        byte,
                    })?
                } else {
                    return Err(XmlParseError::UnknownEntity {
                        name: name.to_string(),
                        byte,
                    });
                };
                let Some(ch) = char::from_u32(code) else {
                    return Err(XmlParseError::Malformed {
                        reason: format!(
                            "the character reference '&{name};' is not a Unicode scalar value"
                        ),
                        byte,
                    });
                };
                // The subset refuses the C0 control characters XML 1.0
                // forbids (only tab, LF, CR may appear as references).
                if code < 0x20 && !matches!(code, 0x9 | 0xA | 0xD) {
                    return Err(XmlParseError::Malformed {
                        reason: format!(
                            "the character reference '&{name};' is a control character \
                             XML 1.0 forbids"
                        ),
                        byte,
                    });
                }
                decoded.push(ch);
            }
        }
        i += semi + 1;
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pulls every event of an input (the test helper: a well-formedness
    /// refusal or the full event list — never a partial success).
    fn events(input: &[u8]) -> Vec<XmlEvent> {
        let mut parser = XmlPullParser::new(input).expect("parser constructs");
        let mut events = Vec::new();
        while let Some(event) = parser.next_event().expect("well-formed") {
            events.push(event);
        }
        events
    }

    fn malformed(input: &[u8]) -> XmlParseError {
        let mut parser = XmlPullParser::new(input).expect("parser constructs");
        loop {
            match parser.next_event() {
                Ok(Some(_)) => continue,
                Ok(None) => panic!("expected a refusal, the document parsed"),
                Err(error) => return error,
            }
        }
    }

    const MINIATURE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <!-- a profile header comment -->\n\
        <rdf:RDF xmlns:cim=\"http://iec.ch/TC57/2013/CIM-schema-cim16#\" \
        xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
        <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
        <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>\n  \
        </cim:BaseVoltage>\n\
        </rdf:RDF>\n";

    #[test]
    fn the_miniature_profile_produces_the_pinned_event_sequence() {
        let parsed = events(MINIATURE.as_bytes());
        assert_eq!(
            parsed.len(),
            7,
            "start RDF, start object, start attribute, text, end attribute, end object, end RDF"
        );
        let XmlEvent::StartElement(root) = &parsed[0] else {
            panic!("expected the root start element");
        };
        assert_eq!(root.name.namespace, RDF_NAMESPACE);
        assert_eq!(root.name.local, "RDF");
        assert!(
            root.attributes.is_empty(),
            "xmlns declarations are consumed into the scope, never emitted as attributes"
        );
        let XmlEvent::StartElement(object) = &parsed[1] else {
            panic!("expected the object start element");
        };
        assert_eq!(
            object.name.namespace,
            "http://iec.ch/TC57/2013/CIM-schema-cim16#",
            "the cim prefix resolves through the root's declaration"
        );
        assert_eq!(object.name.local, "BaseVoltage");
        assert_eq!(
            object.attribute(RDF_NAMESPACE, "ID"),
            Some("_bv20"),
            "the rdf:ID attribute resolves and is entity-decoded"
        );
        let XmlEvent::Text(value) = &parsed[3] else {
            panic!("expected the attribute text");
        };
        assert_eq!(value, "20.0");
        let XmlEvent::EndElement(last) = &parsed[6] else {
            panic!("expected the root end element");
        };
        assert_eq!(last.local, "RDF");
    }

    #[test]
    fn namespace_scoping_resolves_through_enclosing_declarations() {
        let document = b"<root xmlns=\"urn:default\"><child xmlns:p=\"urn:p\" plain=\"v\">\
            <p:leaf p:attr=\"x\"/></child></root>";
        let parsed = events(document);
        let XmlEvent::StartElement(child) = &parsed[1] else {
            panic!("expected the child start element");
        };
        assert_eq!(child.name.namespace, "urn:default", "the default namespace");
        assert_eq!(
            child.attribute("", "plain"),
            Some("v"),
            "an unprefixed attribute is in NO namespace (the default namespace \
             does not apply to attributes)"
        );
        let XmlEvent::StartElement(leaf) = &parsed[2] else {
            panic!("expected the leaf start element");
        };
        assert_eq!(leaf.name.namespace, "urn:p", "the prefix declared on an ancestor");
        assert_eq!(leaf.attribute("urn:p", "attr"), Some("x"));
        let XmlEvent::EndElement(root_end) = &parsed[5] else {
            panic!("expected the root end element");
        };
        assert_eq!(root_end.namespace, "urn:default");
    }

    #[test]
    fn self_closing_elements_deliver_start_then_end() {
        let parsed = events(b"<rdf:RDF><cim:Breaker rdf:ID=\"_brk1\"/></rdf:RDF>");
        assert_eq!(
            parsed.len(),
            4,
            "start root, start object, pending end object, end root"
        );
        assert!(matches!(&parsed[1], XmlEvent::StartElement(_)));
        assert!(matches!(&parsed[2], XmlEvent::EndElement(_)));
        assert!(matches!(&parsed[3], XmlEvent::EndElement(_)));
    }

    #[test]
    fn entities_decode_in_text_and_attribute_values() {
        let parsed = events(
            b"<t a=\"&quot;x&quot;\">&amp;&lt;&gt;&quot;&apos;&#65;&#x42;&#46;</t>",
        );
        let XmlEvent::StartElement(start) = &parsed[0] else {
            panic!("expected the start element");
        };
        assert_eq!(start.attribute("", "a"), Some("\"x\""), "entities in attribute values");
        let XmlEvent::Text(text) = &parsed[1] else {
            panic!("expected the text event");
        };
        assert_eq!(text, "&<>\"AB.", "the five named entities plus decimal and hex references");
    }

    #[test]
    fn whitespace_text_nodes_are_skipped_and_content_text_is_emitted() {
        let parsed = events(b"<t>  left  </t>");
        assert_eq!(parsed.len(), 3, "start, text (whitespace trimmed run has content), end");
        let XmlEvent::Text(text) = &parsed[1] else {
            panic!("expected the text event");
        };
        assert_eq!(text, "  left  ", "the run is emitted verbatim after entity decoding");
        assert_eq!(events(b"<t>   </t>").len(), 2, "a whitespace-only run is skipped");
    }

    #[test]
    fn undeclared_prefixes_are_named_refusals() {
        assert!(matches!(
            malformed(b"<a:b/>"),
            XmlParseError::UndeclaredPrefix { prefix, .. } if prefix == "a"
        ));
        assert!(matches!(
            malformed(b"<r xmlns:p=\"urn:p\"><p:x q:attr=\"v\"/></r>"),
            XmlParseError::UndeclaredPrefix { prefix, .. } if prefix == "q"
        ));
    }

    #[test]
    fn unknown_and_unterminated_entities_are_named_refusals() {
        assert!(matches!(
            malformed(b"<t>&nbsp;</t>"),
            XmlParseError::UnknownEntity { name, .. } if name == "nbsp"
        ));
        assert!(matches!(
            malformed(b"<t>&amp</t>"),
            XmlParseError::Malformed { .. }
        ));
        assert!(
            matches!(malformed(b"<t>&#7;</t>"), XmlParseError::Malformed { .. }),
            "a control character XML 1.0 forbids"
        );
        assert!(matches!(
            malformed(b"<t>&#xZZ;</t>"),
            XmlParseError::UnknownEntity { .. }
        ));
    }

    #[test]
    fn unclosed_elements_are_named_refusals() {
        assert!(matches!(
            malformed(b"<r><a>"),
            XmlParseError::UnclosedElement { name, .. } if name == "a"
        ));
    }

    #[test]
    fn mismatched_close_tags_are_named_refusals() {
        assert!(matches!(
            malformed(b"<r><a></b></r>"),
            XmlParseError::MismatchedCloseTag { expected, found, .. }
                if expected == "a" && found == "b"
        ));
    }

    #[test]
    fn stray_text_outside_the_root_is_a_named_refusal() {
        assert!(matches!(
            malformed(b"junk<r/>"),
            XmlParseError::TextOutsideRoot { .. }
        ));
        assert!(matches!(
            malformed(b"<r/>junk"),
            XmlParseError::TextOutsideRoot { .. }
        ));
        assert_eq!(
            events(b"<r/>   \n").len(),
            2,
            "trailing whitespace after the root is fine (start + end, no text event)"
        );
    }

    #[test]
    fn duplicate_attributes_are_named_refusals_including_expanded_names() {
        assert!(matches!(
            malformed(b"<r a=\"1\" a=\"2\"/>"),
            XmlParseError::DuplicateAttribute { name, .. } if name == "a"
        ));
        // Two prefixes bound to the same URI, same local name: a duplicate
        // on the expanded name.
        assert!(matches!(
            malformed(
                b"<r xmlns:p=\"urn:x\" xmlns:q=\"urn:x\" p:a=\"1\" q:a=\"2\"/>"
            ),
            XmlParseError::DuplicateAttribute { name, .. } if name == "q:a"
        ));
        assert!(matches!(
            malformed(b"<r xmlns:p=\"urn:1\" xmlns:p=\"urn:2\"/>"),
            XmlParseError::DuplicateAttribute { name, .. } if name == "xmlns:p"
        ));
    }

    #[test]
    fn cdata_dtd_and_xinclude_are_named_refusals_never_silent_skips() {
        assert!(matches!(
            malformed(b"<r><![CDATA[data]]></r>"),
            XmlParseError::CdataUnsupported { .. }
        ));
        assert!(matches!(
            malformed(b"<!DOCTYPE rdf:RDF><r/>"),
            XmlParseError::DtdUnsupported { .. }
        ));
        assert!(matches!(
            malformed(
                b"<r xmlns:xi=\"http://www.w3.org/2001/XInclude\"><xi:include href=\"x\"/></r>"
            ),
            XmlParseError::XIncludeUnsupported { .. }
        ));
    }

    #[test]
    fn empty_input_markup_after_root_and_bad_values_are_named_refusals() {
        assert!(
            matches!(malformed(b""), XmlParseError::Malformed { .. }),
            "empty input has no root element"
        );
        assert!(
            matches!(malformed(b"<r/><s/>"), XmlParseError::Malformed { .. }),
            "markup after the root closed"
        );
        assert!(
            matches!(malformed(b"<r a=\"<x>\"/>"), XmlParseError::Malformed { .. }),
            "a raw '<' inside an attribute value"
        );
        assert!(
            matches!(malformed(b"<r a=x/>"), XmlParseError::Malformed { .. }),
            "an unquoted attribute value"
        );
        assert!(
            matches!(malformed(b"<1r/>"), XmlParseError::Malformed { .. }),
            "a name must start with a letter or '_'"
        );
    }

    #[test]
    fn the_bounded_input_guard_refuses_before_any_scan() {
        // Heap-allocated (a 64 MiB + 1 stack array would overflow a test
        // thread's stack at the human gate).
        let input = vec![b'<'; MAX_XML_INPUT_BYTES + 1];
        assert!(matches!(
            XmlPullParser::new(&input),
            Err(XmlParseError::InputTooLarge {
                max_bytes,
                observed_bytes
            }) if max_bytes == MAX_XML_INPUT_BYTES
                && observed_bytes == MAX_XML_INPUT_BYTES + 1
        ));
        // The explicit limit (the pinned-test seam) fires the same way.
        assert!(matches!(
            XmlPullParser::with_max_bytes(b"<r><a/></r>", 8),
            Err(XmlParseError::InputTooLarge { .. })
        ));
        assert!(XmlPullParser::with_max_bytes(b"<r><a/></r>", 11).is_ok());
    }

    #[test]
    fn invalid_utf8_is_a_named_refusal() {
        assert!(matches!(
            XmlPullParser::new(&[b'<', 0xFF, b'/'][..]),
            Err(XmlParseError::InvalidUtf8 { .. })
        ));
    }
}
