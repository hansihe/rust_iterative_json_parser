use ::PResult;
use ::input::Pos;
use ::tokenizer::Token;

pub mod string;

pub enum SourceError<Bail> {
    Bail(Bail),
    Eof,
}

pub trait Source {
    type Bail;

    fn position(&self) -> Pos;
    fn skip(&mut self, num: usize);
    fn peek_char(&self) -> Result<u8, SourceError<Self::Bail>>;
}
