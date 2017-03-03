use ::input::Pos;

#[derive(Debug, PartialEq)]
pub enum ParseError<SourceBail> {
    Unexpected(Pos, Unexpected),
    End,

    // Indicators from Source.
    // Does not actually signal errors.
    Eof,
    SourceBail(SourceBail),
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Unexpected {
    // Parser
    ObjectOpen,
    ObjectClose,
    ArrayOpen,
    ArrayClose,
    Comma,
    Colon,
    Exponent,
    Dot,
    Sign,
    Number,
    Bool,
    Null,
    Quote,

    // Tokenizer
    InvalidUtf8,
    InvalidEscape,
    InvalidEscapeHex,
    Character,
    Eof,
}

use self::Unexpected as U;

impl Unexpected {
    pub fn explain(self) -> &'static str {
        match self {
            U::ObjectOpen => "unexpected {",
            U::ObjectClose => "unexpected }",
            U::ArrayOpen => "unexpected [",
            U::ArrayClose => "unexpected ]",
            U::Comma => "unexpected ,",
            U::Colon => "unexpected :",
            U::Exponent => "unexpected E",
            U::Dot => "unexpected .",
            U::Sign => "unexpected - or +",
            U::Number => "unexpected number",
            U::Bool => "unexpected boolean",
            U::Null => "unexpected null",
            U::Quote => "unexpected \"",

            U::InvalidUtf8 => "expected valid utf8 data",
            U::InvalidEscape => "expected one of \"\\ubfnrt\"",
            U::InvalidEscapeHex => "expected hexidecimal",
            U::Character => "unexpected character",
            U::Eof => "unexpected EOF",
        }
    }
}
