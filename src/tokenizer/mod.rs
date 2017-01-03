use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum Token {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    ObjectOpen,
    ObjectClose,
    ArrayOpen,
    ArrayClose,
    Comma,
    Colon,
    Eof,
}

pub trait Tokenizer: Debug {
    fn token(&mut self) -> Token;
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
    fn token(&mut self) -> Token {
        self.tokens.pop().unwrap()
    }
}
