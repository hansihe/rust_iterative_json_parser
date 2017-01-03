type PResult<T> = Result<T, &'static str>;

mod parser;
mod tokenizer;
mod sink;

use tokenizer::{Token, TestTokenStream};
use parser::Parser;
use sink::into_enum::EnumSink;

fn test_parse() -> PResult<()> {
    let tokenizer = TestTokenStream::new(
        vec![
            Token::Eof,
            Token::ObjectClose,
            Token::ArrayClose,
            Token::String("bar".to_string()),
            Token::Comma,
            Token::Number(1.5f64),
            Token::Comma,
            Token::Null,
            Token::Comma,
            Token::Boolean(true),
            Token::ArrayOpen,
            Token::Colon,
            Token::String("foo".to_string()),
            Token::ObjectOpen,
        ]);

    let mut parser = Parser::new(Box::new(tokenizer));

    let mut sink = EnumSink::new();

    loop {
        match parser.step(&mut sink) {
            Ok(true) => {
                println!("{:?}", sink.to_result());
                return Ok(());
            },
            Ok(false) => continue,
            Err(msg) => {
                println!("Error: \n{:?}\n{:?}", msg, parser);
                return Err(msg);
            }
        }
    }
}

fn main() {
    test_parse();
}
