use ::input::Pos;
use ::tokenizer::Token;

#[derive(Debug, PartialEq)]
pub enum ParseError<SourceBail, SinkBail> {
    Unexpected(Pos),
    UnexpectedToken(Pos, Token),

    // Indicators from Source.
    // Does not actually signal errors.
    Eof,
    SourceBail(SourceBail),
    SinkBail(SinkBail),
}
