use super::{Source, PeekResult};
use ::Bailable;
use ::PResult;
use ::input::Pos;
use ::error::ParseError;
use ::std::sync::Mutex;

#[derive(Debug)]
pub struct VecSource {
    vec: Vec<u8>,
    pos: usize,
}

impl VecSource {

    pub fn new(vec: Vec<u8>) -> VecSource {
        VecSource {
            vec: vec,
            pos: 0,
        }
    }

}

impl Bailable for VecSource {
    type Bail = ();
}

impl Source for VecSource {

    fn position(&self) -> Pos {
        self.pos.into()
    }

    fn skip(&mut self, num: usize) {
        self.pos += num;
    }

    fn peek_char(&self) -> PeekResult<Self::Bail> {
        if self.pos >= self.vec.len() {
            PeekResult::Eof
        } else {
            let character = self.vec[self.pos];
            PeekResult::Ok(character)
        }
    }

    fn peek_slice<'a>(&'a self, length: usize) -> Option<&'a [u8]> {
        let pos = self.pos;
        self.vec.get(pos..(pos+length))
    }

}

#[derive(Debug)]
pub struct VecSourceB {
    vec: Vec<u8>,
    pos: Mutex<usize>,
}

impl VecSourceB {

    pub fn new(vec: Vec<u8>) -> VecSourceB {
        VecSourceB {
            vec: vec,
            pos: Mutex::new(0),
        }
    }

}

impl Bailable for VecSourceB {
    type Bail = ();
}

impl Source for VecSourceB {

    fn position(&self) -> Pos {
        (*self.pos.lock().unwrap()).into()
    }

    fn skip(&mut self, num: usize) {
        *self.pos.lock().unwrap() += num;
    }

    fn peek_char(&self) -> PeekResult<Self::Bail> {
        let mut pos = self.pos.lock().unwrap();

        if *pos >= self.vec.len() {
            PeekResult::Eof
        } else {
            let character = self.vec[*pos];
            if character == b'&' {
                *pos += 1;
                PeekResult::Bail(())
            } else {
                PeekResult::Ok(character)
            }
        }
    }

    fn peek_slice<'a>(&'a self, _length: usize) -> Option<&'a [u8]> {
        None
    }

}
