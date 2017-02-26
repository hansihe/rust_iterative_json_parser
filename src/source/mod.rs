use ::PResult;
use ::Bailable;
use ::input::Pos;

pub mod string;

pub enum PeekResult<Bail> {
    Ok(u8),
    Bail(Bail),
    Eof,
}

pub trait Source: Bailable {
    fn position(&self) -> Pos;
    fn skip(&mut self, num: usize);
    fn peek_char(&mut self) -> PeekResult<Self::Bail>;
    fn peek_slice<'a>(&'a self, length: usize) -> Option<&'a [u8]>;
}
