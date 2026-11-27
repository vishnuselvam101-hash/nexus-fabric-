//! Tiny JSON writer (stable key order as given, 2-space indent).

pub enum J {
    Null,
    Raw(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

fn esc(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
}

impl J {
    pub fn write(&self, o: &mut String, depth: usize) {
        let pad = |o: &mut String, d: usize| o.push_str(&"  ".repeat(d));
        match self {
            J::Null => o.push_str("null"),
            J::Raw(r) => o.push_str(if r.is_empty() { "null" } else { r }),
            J::Str(s) => esc(s, o),
            J::Arr(a) if a.is_empty() => o.push_str("[]"),
            J::Obj(m) if m.is_empty() => o.push_str("{}"),
            J::Arr(a) => {
                o.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    pad(o, depth + 1);
                    v.write(o, depth + 1);
                    o.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
                }
                pad(o, depth);
                o.push(']');
            }
            J::Obj(m) => {
                o.push_str("{\n");
                for (i, (k, v)) in m.iter().enumerate() {
                    pad(o, depth + 1);
                    esc(k, o);
                    o.push_str(": ");
                    v.write(o, depth + 1);
                    o.push_str(if i + 1 < m.len() { ",\n" } else { "\n" });
                }
                pad(o, depth);
                o.push('}');
            }
        }
    }
}
