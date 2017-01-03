pub mod debug_print;
pub mod into_enum;

pub trait ParserSink {
    fn push_map(&mut self);
    fn push_array(&mut self);
    fn push_string(&mut self, string: &str);
    fn push_number(&mut self, integer: f64);
    fn push_bool(&mut self, boolean: bool);
    fn push_null(&mut self);
    fn finalize_array(&mut self);
    fn finalize_map(&mut self);
    fn pop_into_map(&mut self);
    fn pop_into_array(&mut self);
}


