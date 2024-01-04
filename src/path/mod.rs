//!
//! > ```bnf
//! >    path (or "remote-name")  := path-component ['/' path-component]*
//! >    path-component           := alpha-numeric [separator alpha-numeric]*
//! >    alpha-numeric            := /[a-z0-9]+/
//! >    separator                := /[_.]|__|[-]*/
//! > ```
//!
//!
//!
//! https://github.com/distribution/reference/blob/v0.5.0/reference.go#L7-L16
//! > Throughout this document, <name> MUST match the following regular expression:
//! > ```ebnf
//! > [a-z0-9]+([._-][a-z0-9]+)*(/[a-z0-9]+([._-][a-z0-9]+)*)*
//! > [a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(\/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*
//! > ```
//! > -- https://github.com/opencontainers/distribution-spec/blob/v1.0.1/spec.md#pulling-manifests
//! > -- https://github.com/opencontainers/distribution-spec/blob/v1.1.0-rc3/spec.md#pulling-manifests
//! > -- https://github.com/opencontainers/distribution-spec/commit/a73835700327bd1c037e33d0834c46ff98ac1286
//! > -- https://github.com/opencontainers/distribution-spec/commit/efe2de09470d7f182d2fbd83ac4462fbdc462455

type U = u8; // HACK: arbitrary limit
trait ParseStatus {
    fn ok(self, err: Self) -> Result<Self, Self>;
}
impl ParseStatus for U {
    fn ok(self, err: Self) -> Result<Self, Self> {
        if self == 0 || self == U::MAX {
            Err(self + err)
        } else {
            Ok(self)
        }
    }
}

fn separator(src: &str) -> U {
    let mut chars = src.chars();
    if let Some(first) = chars.next() {
        match first {
            '.' => return 1,
            '_' => {
                if chars.next().map(|c| c == '_').unwrap_or(false) {
                    return 2; // "__"
                } else {
                    return 1; // "_"
                }
            }
            '-' => {
                let mut len = 1;
                for c in chars {
                    match c {
                        '-' => {
                            if len == U::MAX {
                                // guard against overflow
                                break;
                            }
                            len += 1
                        }
                        _ => break,
                    }
                }
                return len;
            }
            _ => return 0,
        }
    } else {
        return 0;
    }
}

fn alpha_numeric(src: &str) -> U {
    let mut len = 0;
    for c in src.chars() {
        match c {
            'a'..='z' | '0'..='9' => {
                if len == U::MAX {
                    // guard against overflow
                    break;
                }
                len += 1
            }
            _ => break,
        }
    }
    len
}

fn component(src: &str) -> Result<U, U> {
    let mut len = alpha_numeric(src).ok(0)?;
    loop {
        if let Ok(sep) = separator(&src[len as usize..]).ok(len) {
            len += sep;
            len += alpha_numeric(&src[len as usize..]).ok(len)?;
        } else {
            break;
        }
    }
    Ok(len)
}

fn path(src: &str) -> Result<U, U> {
    let mut len = component(src)?;
    loop {
        if let Some('/') = &src[len as usize..].chars().next() {
            len += 1;
            len += component(&src[len as usize..]).map_err(|e| e + len)?;
        } else {
            break;
        }
    }
    Ok(len)
}
