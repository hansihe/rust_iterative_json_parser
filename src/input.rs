use ::parser::NumberData;
use ::source::{Source, PeekResult};
use ::sink::Sink;

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

    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    pub fn size(&self) -> usize {
        self.end - self.start
    }
}

pub trait Bailable {
    type Bail;
}

#[derive(Debug, PartialEq, Eq)]
pub enum BailVariant<SourceBail, SinkBail> {
    Source(SourceBail),
    Sink(SinkBail),
}

pub struct SourceSink<Src, Snk>
    where Src: Source,
          Snk: Sink
{
    pub source: Src,
    pub sink: Snk,
}

impl<Src, Snk> Bailable for SourceSink<Src, Snk>
    where Src: Source,
          Snk: Sink
{
    type Bail = BailVariant<Src::Bail, Snk::Bail>;
}

impl<Src, Snk> Source for SourceSink<Src, Snk>
    where Src: Source,
          Snk: Sink
{
    #[inline(always)]
    fn position(&self) -> Pos {
        self.source.position()
    }
    #[inline(always)]
    fn skip(&mut self, num: usize) {
        self.source.skip(num)
    }
    #[inline(always)]
    fn peek_char(&mut self) -> PeekResult<BailVariant<Src::Bail, Snk::Bail>> {
        match self.source.peek_char() {
            PeekResult::Ok(num) => PeekResult::Ok(num),
            PeekResult::Eof => PeekResult::Eof,
            PeekResult::Bail(bail) => {
                PeekResult::Bail(BailVariant::Source(bail))
            },
        }
    }
    #[inline(always)]
    fn peek_slice<'a>(&'a self, length: usize) -> Option<&'a [u8]> {
        self.source.peek_slice(length)
    }
}

impl<Src, Snk> Sink for SourceSink<Src, Snk>
    where Src: Source,
          Snk: Sink
{
    #[inline(always)]
    fn push_map(&mut self) {
        self.sink.push_map()
    }
    #[inline(always)]
    fn push_array(&mut self) {
        self.sink.push_array()
    }
    #[inline(always)]
    fn push_number(&mut self, integer: NumberData) {
        self.sink.push_number(integer)
    }
    #[inline(always)]
    fn push_bool(&mut self, boolean: bool) {
        self.sink.push_bool(boolean)
    }
    #[inline(always)]
    fn push_null(&mut self) {
        self.sink.push_null()
    }

    #[inline(always)]
    fn start_string(&mut self) {
        self.sink.start_string()
    }
    #[inline(always)]
    fn append_string_range(&mut self, string: Range) {
        self.sink.append_string_range(string)
    }
    #[inline(always)]
    fn append_string_single(&mut self, character: u8) {
        self.sink.append_string_single(character)
    }
    #[inline(always)]
    fn append_string_codepoint(&mut self, codepoint: char) {
        self.sink.append_string_codepoint(codepoint)
    }
    #[inline(always)]
    fn finalize_string(&mut self) {
        self.sink.finalize_string()
    }

    #[inline(always)]
    fn finalize_array(&mut self) {
        self.sink.finalize_array()
    }
    #[inline(always)]
    fn finalize_map(&mut self) {
        self.sink.finalize_map()
    }
    #[inline(always)]
    fn pop_into_array(&mut self) {
        self.sink.pop_into_array()
    }
    #[inline(always)]
    fn pop_into_map(&mut self) {
        self.sink.pop_into_map()
    }
}
