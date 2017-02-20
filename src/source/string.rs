use super::{Source, SourceError};
use ::PResult;
use ::input::Pos;
use ::error::ParseError;
use ::std::sync::Mutex;

#[derive(Debug)]
pub struct VecSource {
    vec: Vec<u8>,
    pos: Mutex<usize>,
}

impl VecSource {

    pub fn new(vec: Vec<u8>) -> VecSource {
        VecSource {
            vec: vec,
            pos: Mutex::new(0),
        }
    }

}

impl Source for VecSource {
    type Bail = ();

    fn position(&self) -> Pos {
        (*self.pos.lock().unwrap()).into()
    }

    fn skip(&mut self, num: usize) {
        *self.pos.lock().unwrap() += num;
    }

    fn peek_char(&self) -> Result<char, SourceError<Self::Bail>> {
        let mut pos = self.pos.lock().unwrap();

        if *pos >= self.vec.len() {
            Err(SourceError::Eof)
        } else {
            let character = self.vec[*pos] as char;
            if character == '&' {
                *pos += 1;
                Err(SourceError::Bail(()))
            } else {
                Ok(character)
            }
        }
    }

}
