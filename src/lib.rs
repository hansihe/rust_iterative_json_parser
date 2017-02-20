pub mod parser;
pub mod tokenizer;
pub mod sink;
pub mod input;
pub mod error;
pub mod source;
pub mod decoder;

pub type PResult<T, SourceBail, SinkBail> = Result<T, error::ParseError<SourceBail, SinkBail>>;

pub use parser::{ParserState, NumberData};
pub use ParserState as Parser;
pub use source::{Source, SourceError};
pub use sink::Sink;
pub use error::ParseError;
pub use input::{Range, Pos};
