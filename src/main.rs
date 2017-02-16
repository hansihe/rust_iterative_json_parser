type PResult<T> = Result<T, error::ParseError>;

mod parser;
mod tokenizer;
mod sink;
mod input;
mod error;
mod source;

fn main() {
}
