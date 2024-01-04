// > Grammar
// >
// > ```ebnf
// > reference                       := name [ ":" tag ] [ "@" digest ]
// > name                            := [domain '/'] remote-name
// > domain                          := host [':' port-number]
// > host                            := domain-name | IPv4address | \[ IPv6address \] ; rfc3986 appendix-A
// > domain-name                     := domain-component ['.' domain-component]*
// > domain-component                := /([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9])/
// > port-number                     := /[0-9]+/
// > path-component                  := alpha-numeric [separator alpha-numeric]*
// > path (or "remote-name")         := path-component ['/' path-component]*
// > alpha-numeric                   := /[a-z0-9]+/
// > separator                       := /[_.]|__|[-]*/
// >
// > tag                             := /[\w][\w.-]{0,127}/
// >
// > digest                          := digest-algorithm ":" digest-hex
// > digest-algorithm                := digest-algorithm-component [ digest-algorithm-separator digest-algorithm-component ]*
// > digest-algorithm-separator      := /[+.-_]/
// > digest-algorithm-component      := /[A-Za-z][A-Za-z0-9]*/
// > digest-hex                      := /[0-9a-fA-F]{32,}/ ; At least 128 bit digest value
// >
// > identifier                      := /[a-f0-9]{64}/
// > ```
// >
// > -- https://github.com/distribution/reference/blob/v0.5.0/reference.go#L4-L26
// > -- https://github.com/distribution/reference/blob/4894124079e525c3c3c5c8aacaa653b5499004e9/reference.go#L4-L26

// https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention

pub mod digest;
mod parse;
mod path;
