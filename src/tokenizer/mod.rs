use std::fmt::Debug;
use ::input::{Pos, Range};
use ::error::ParseError;

pub mod basic;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Token {
    // Number parts
    Sign(bool),
    Number(Range),
    Dot,
    Exponent,

    // String parts
    Quote,
    StringSource(Range),
    StringSingle(u8),

    // Rest
    Boolean(bool),
    Null,
    ObjectOpen,
    ObjectClose,
    ArrayOpen,
    ArrayClose,
    Comma,
    Colon,

    // Special
    Eof,
}

pub struct TokenSpan{
    pub token: Token,
    pub span: Range,
}

pub trait Tokenizer: Debug {
    fn token(&mut self) -> Result<Token, ParseError>;
    fn position(&self) -> Pos;
}

#[derive(Debug)]
pub struct TestTokenStream {
    tokens: Vec<Token>,
}

impl TestTokenStream {
    pub fn new(tokens: Vec<Token>) -> Self {
        TestTokenStream {
            tokens: tokens,
        }
    }
}

impl Tokenizer for TestTokenStream {
    fn token(&mut self) -> Result<Token, ParseError> {
        Ok(self.tokens.pop().unwrap())
    }

    fn position(&self) -> Pos {
        self.tokens.len().into()
    }
}
