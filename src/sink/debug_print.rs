use ::Bailable;
use super::{Sink, NumberData, Position, StringPosition};
use ::input::Range;

pub struct PrintSink {}

impl PrintSink {
    pub fn new() -> PrintSink {
        PrintSink {}
    }
}

impl Bailable for PrintSink {
    type Bail = ();
}

impl Sink for PrintSink {
    fn push_map(&mut self, _pos: Position) {
        println!("push_map");
    }
    fn push_array(&mut self, _pos: Position) {
        println!("push_array");
    }
    fn push_number(&mut self, _pos: Position, num: NumberData) -> Result<(), Self::Bail> {
        println!("push_float {:?}", num);
        Ok(())
    }
    fn push_bool(&mut self, _pos: Position, val: bool) -> Result<(), Self::Bail> {
        println!("push_bool {:?}", val);
        Ok(())
    }
    fn push_null(&mut self, _pos: Position) -> Result<(), Self::Bail> {
        println!("push_none");
        Ok(())
    }

    fn start_string(&mut self, _pos: StringPosition) {
        println!("start_string")
    }
    fn append_string_range(&mut self, string: Range) {
        println!("append_string_range {:?}", string)
    }
    fn append_string_single(&mut self, character: u8) {
        println!("append_string_single {:?}", character)
    }
    fn append_string_codepoint(&mut self, codepoint: char) {
        println!("append_string_codepoint {:?}", codepoint)
    }
    fn finalize_string(&mut self, _pos: StringPosition) -> Result<(), Self::Bail> {
        println!("finalize_string");
        Ok(())
    }

    fn finalize_map(&mut self, _pos: Position) -> Result<(), Self::Bail> {
        println!("finalize_map");
        Ok(())
    }
    fn finalize_array(&mut self, _pos: Position) -> Result<(), Self::Bail> {
        println!("finalize_array");
        Ok(())
    }
    fn pop_into_map(&mut self) {
        println!("pop_into_map");
    }
    fn pop_into_array(&mut self) {
        println!("pop_into_array");
    }
}
