use super::ParserSink;

pub struct PrintSink {}

impl PrintSink {
    fn new() -> PrintSink {
        PrintSink {}
    }
}

impl ParserSink for PrintSink {
    fn push_map(&mut self) { println!("push_map"); }
    fn push_array(&mut self) { println!("push_array"); }
    fn push_string(&mut self, string: &str) { println!("push_string {:?}", string); }
    fn push_number(&mut self, num: f64) { println!("push_float {:?}", num); }
    fn push_bool(&mut self, val: bool) { println!("push_bool {:?}", val); }
    fn push_null(&mut self) { println!("push_none"); }
    fn finalize_map(&mut self) { println!("finalize_map"); }
    fn finalize_array(&mut self) { println!("finalize_array"); }
    fn pop_into_map(&mut self) { println!("pop_into_map"); }
    fn pop_into_array(&mut self) { println!("pop_into_array"); }
}
