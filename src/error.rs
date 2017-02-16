use ::input::Pos;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedChar {
        pos: Pos,
        expected: Option<char>,
    },
    UnexpectedToken {
        pos: Pos,
        message: &'static str,
    },
    UnexpectedEof,
    ExpectedEof,

    // Indicators from Source.
    // Does not actually signal errors.
    Eof,
    Bail,
}
