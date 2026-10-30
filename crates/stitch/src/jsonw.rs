//! Writing mod.json: a value tree, rendered with the top level one key a line and everything under it on
//! that line (modcheck's parser reads it back).

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    S(String),
    N(f64),
    A(Vec<J>),
    O(Vec<(String, J)>),
}

pub fn s(v: impl Into<String>) -> J {
    J::S(v.into())
}

pub fn o(kv: Vec<(&str, J)>) -> J {
    J::O(kv.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn number(n: f64) -> String {
    if n.is_finite() {
        // Shortest text that reads back to the same f64.
        let t = format!("{n}");
        if t.contains('e') { format!("{n:e}") } else { t }
    } else {
        "null".into()
    }
}

impl J {
    pub fn compact(&self) -> String {
        match self {
            J::S(s) => quote(s),
            J::N(n) => number(*n),
            J::A(a) => format!("[{}]", a.iter().map(J::compact).collect::<Vec<_>>().join(", ")),
            J::O(kv) => format!(
                "{{{}}}",
                kv.iter().map(|(k, v)| format!("{}: {}", quote(k), v.compact())).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// The top level one key a line; a list of objects one object a line.
    pub fn pretty(&self) -> String {
        let J::O(kv) = self else { return self.compact() + "\n" };
        let mut out = String::from("{\n");
        for (i, (k, v)) in kv.iter().enumerate() {
            let body = match v {
                J::A(a) if a.iter().any(|x| matches!(x, J::O(_))) && !a.is_empty() => format!(
                    "[\n{}\n  ]",
                    a.iter().map(|x| format!("    {}", x.compact())).collect::<Vec<_>>().join(",\n")
                ),
                J::A(a) if a.iter().all(|x| matches!(x, J::S(_))) && a.len() > 1 => format!(
                    "[\n{}\n  ]",
                    a.iter().map(|x| format!("    {}", x.compact())).collect::<Vec<_>>().join(",\n")
                ),
                v => v.compact(),
            };
            out.push_str(&format!("  {}: {}{}\n", quote(k), body, if i + 1 < kv.len() { "," } else { "" }));
        }
        out.push_str("}\n");
        out
    }
}
