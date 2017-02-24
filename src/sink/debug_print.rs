use super::{Sink, NumberData};
use ::input::Range;

pub struct PrintSink {}

impl PrintSink {
    pub fn new() -> PrintSink {
        PrintSink {}
    }
}

impl Sink for PrintSink {
    type Bail = ();

    fn push_map(&mut self) {
        println!("push_map");
    }
    fn push_array(&mut self) {
        println!("push_array");
    }
    // fn push_string(&mut self, string: Range) { println!("push_string {:?}", string); }
    fn push_number(&mut self, num: NumberData) -> Result<(), Self::Bail> {
        println!("push_float {:?}", num);
        Ok(())
    }
    fn push_bool(&mut self, val: bool) {
        println!("push_bool {:?}", val);
    }
    fn push_null(&mut self) {
        println!("push_none");
    }

    fn start_string(&mut self) {
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
    fn finalize_string(&mut self) {
        println!("finalize_string")
    }

    fn finalize_map(&mut self) {
        println!("finalize_map");
    }
    fn finalize_array(&mut self) {
        println!("finalize_array");
    }
    fn pop_into_map(&mut self) {
        println!("pop_into_map");
    }
    fn pop_into_array(&mut self) {
        println!("pop_into_array");
    }
}
