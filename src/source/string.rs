use super::{Source, PeekResult};
use ::Bailable;
use ::input::Pos;

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

    fn peek_char(&mut self) -> PeekResult<Self::Bail> {
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
    pos: usize,
    bailed: bool,
}

impl VecSourceB {

    pub fn new(vec: Vec<u8>) -> VecSourceB {
        VecSourceB {
            vec: vec,
            pos: 0,
            bailed: false,
        }
    }

}

impl Bailable for VecSourceB {
    type Bail = ();
}

impl Source for VecSourceB {

    fn position(&self) -> Pos {
        self.pos.into()
    }

    fn skip(&mut self, num: usize) {
        self.pos += num;
        self.bailed = false;
    }

    fn peek_char(&mut self) -> PeekResult<Self::Bail> {
        if !self.bailed {
            self.bailed = true;
            PeekResult::Bail(())
        } else {
            let pos = self.pos;

            if pos >= self.vec.len() {
                PeekResult::Eof
            } else {
                PeekResult::Ok(self.vec[pos])
            }
        }
    }

    fn peek_slice<'a>(&'a self, _length: usize) -> Option<&'a [u8]> {
        None
    }

}
