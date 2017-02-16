type PResult<T> = Result<T, error::ParseError>;

pub mod parser;
pub mod tokenizer;
pub mod sink;
pub mod input;
pub mod error;
pub mod source;
