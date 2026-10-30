//! Just enough JSON for `mod.json`: the shell's track mods, world mods and the stitcher read it the
//! same way (P232: moved here from the shell's `track_mods.rs`, which re-exports it).

/// Just enough JSON for `mod.json`: numbers are kept as the text they were written as, so a
/// spawn's coordinates reach the scene's own float conversion untouched.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { s: text.as_bytes(), at: 0, depth: 0 };
        let v = p.value()?;
        p.space();
        if p.at != p.s.len() {
            return Err(format!("text after the value at byte {}", p.at));
        }
        Ok(v)
    }

    /// An object's field; the last one where a key is written twice.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<&str> {
        match self {
            Json::Num(s) => Some(s),
            _ => None,
        }
    }

    /// A number as a double (the stitcher's reading; the shell keeps the text, [`Json::num`]).
    pub fn f64(&self) -> Option<f64> {
        self.num().and_then(|n| n.parse().ok())
    }

    pub fn arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.at < self.s.len() && matches!(self.s[self.at], b' ' | b'\t' | b'\r' | b'\n') {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn eat(&mut self, b: u8) -> Result<(), String> {
        self.space();
        if self.peek() == Some(b) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", b as char, self.at))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        self.depth += 1;
        if self.depth > 64 {
            return Err("nested too deep".into());
        }
        let v = match self.peek().ok_or("the text ends where a value should be")? {
            b'{' => {
                self.at += 1;
                let mut fields = Vec::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                } else {
                    loop {
                        self.space();
                        let key = self.string()?;
                        self.eat(b':')?;
                        fields.push((key, self.value()?));
                        self.space();
                        match self.peek() {
                            Some(b',') => self.at += 1,
                            Some(b'}') => {
                                self.at += 1;
                                break;
                            }
                            _ => return Err(format!("expected , or }} at byte {}", self.at)),
                        }
                    }
                }
                Json::Obj(fields)
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                } else {
                    loop {
                        items.push(self.value()?);
                        self.space();
                        match self.peek() {
                            Some(b',') => self.at += 1,
                            Some(b']') => {
                                self.at += 1;
                                break;
                            }
                            _ => return Err(format!("expected , or ] at byte {}", self.at)),
                        }
                    }
                }
                Json::Arr(items)
            }
            b'"' => Json::Str(self.string()?),
            b't' => self.word("true", Json::Bool(true))?,
            b'f' => self.word("false", Json::Bool(false))?,
            b'n' => self.word("null", Json::Null)?,
            b'-' | b'0'..=b'9' => {
                let from = self.at;
                while self.at < self.s.len() && matches!(self.s[self.at], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.s[from..self.at]).map_err(|e| e.to_string())?;
                text.parse::<f64>().map_err(|_| format!("{text:?} is not a number"))?;
                Json::Num(text.to_string())
            }
            b => return Err(format!("unexpected {:?} at byte {}", b as char, self.at)),
        };
        self.depth -= 1;
        Ok(v)
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.s[self.at..].starts_with(w.as_bytes()) {
            self.at += w.len();
            Ok(v)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected a string at byte {}", self.at));
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            let b = self.peek().ok_or("a string runs to the end of the text")?;
            self.at += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or("a string runs to the end of the text")?;
                    self.at += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&hi) && self.s[self.at..].starts_with(b"\\u") {
                                self.at += 2;
                                let lo = self.hex4()?;
                                0x10000 + ((hi - 0xd800) << 10) + (lo.wrapping_sub(0xdc00) & 0x3ff)
                            } else {
                                hi
                            };
                            char::from_u32(code).unwrap_or('\u{fffd}')
                        }
                        _ => return Err(format!("a bad escape at byte {}", self.at)),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err(format!("a control character in a string at byte {}", self.at)),
                b => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| "a string is not UTF-8".to_string())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.s.get(self.at..self.at + 4).ok_or("a \\u escape runs to the end of the text")?;
        let text = std::str::from_utf8(digits).map_err(|e| e.to_string())?;
        let v = u32::from_str_radix(text, 16).map_err(|_| format!("a bad \\u escape at byte {}", self.at))?;
        self.at += 4;
        Ok(v)
    }
}
