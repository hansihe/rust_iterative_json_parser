#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Pos(pub usize);

impl From<usize> for Pos {
    fn from(num: usize) -> Pos {
        Pos(num)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Range {

    pub fn new(start: Pos, end: Pos) -> Range {
        Range {
            start: start.0,
            end: end.0,
        }
    }

}
