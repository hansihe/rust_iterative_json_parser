use ::input::Range;
pub use ::parser::NumberData;

pub mod debug_print;
pub mod into_enum;

pub trait Sink {
    type Bail;

    fn push_map(&mut self);
    fn push_array(&mut self);
    fn push_number(&mut self, integer: NumberData);
    fn push_bool(&mut self, boolean: bool);
    fn push_null(&mut self);

    fn start_string(&mut self);
    fn append_string_range(&mut self, string: Range);
    fn append_string_single(&mut self, character: u8);
    fn append_string_codepoint(&mut self, codepoint: u32);
    fn finalize_string(&mut self);

    fn finalize_array(&mut self);
    fn finalize_map(&mut self);
    fn pop_into_map(&mut self);
    fn pop_into_array(&mut self);
}


