use ::PResult;
use ::input::Pos;
use ::tokenizer::Token;

pub mod string;

pub trait Source {
    fn position(&self) -> Pos;
    fn skip(&mut self, num: usize);
    fn peek_char(&self) -> PResult<char>;
}
