use ::input::Pos;

#[derive(Debug, PartialEq)]
pub enum ParseError<SourceBail> {
    Unexpected(Pos),

    // Indicators from Source.
    // Does not actually signal errors.
    Eof,
    SourceBail(SourceBail),
}
