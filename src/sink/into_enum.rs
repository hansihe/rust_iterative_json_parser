use super::{Sink, NumberData};
use ::input::Range;

#[derive(Debug, PartialEq)]
pub enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    String(String),
    // To simplify testing, numbers are converted back into
    // strings. In a real-life implementation this would be
    // converted into an integer or float using some unspecified
    // algorithm.
    Number(String),
    Boolean(bool),
    Null,
}

/// Sink that puts all values into an enum.
/// Intended for testing, copies like crazy.
#[derive(Debug)]
pub struct EnumSink {
    stack: Vec<Json>,
    source: &'static [u8],
    current_string: Vec<u8>,
}

impl EnumSink {
    pub fn new(source: &'static [u8]) -> EnumSink {
        EnumSink {
            stack: vec![],
            source: source,
            current_string: Vec::new(),
        }
    }

    fn range_to_str<'a>(&'a mut self, range: Range) -> &'a str {
        let raw = &self.source[(range.start)..(range.end)];
        ::std::str::from_utf8(raw).unwrap()
    }

    pub fn to_result(mut self) -> Json {
        if self.stack.len() != 1 {
            panic!("Result not ready.");
        }
        self.stack.pop().unwrap()
    }
}

impl Sink for EnumSink {
    type Bail = ();
    fn push_map(&mut self) { self.stack.push(Json::Object(vec![])) }
    fn push_array(&mut self) { self.stack.push(Json::Array(vec![])) }
    fn push_number(&mut self, number: NumberData) {
        let mut out = String::new();

        if number.sign { out.push('+'); }
        else { out.push('-'); }

        out.push_str(self.range_to_str(number.integer));

        out.push('.');

        if let Some(range) = number.decimal { out.push_str(self.range_to_str(range)); }
        else { out.push('0'); }

        out.push('e');

        if number.exponent_sign { out.push('+'); }
        else { out.push('-'); }

        if let Some(range) = number.exponent { out.push_str(self.range_to_str(range)); }
        else { out.push('1') }

        self.stack.push(Json::Number(out))
    }
    fn push_bool(&mut self, boolean: bool) { self.stack.push(Json::Boolean(boolean)) }
    fn push_null(&mut self) { self.stack.push(Json::Null) }

    fn start_string(&mut self) {}
    fn append_string_range(&mut self, string: Range) {
        let range = &self.source[(string.start)..(string.end)];
        self.current_string.extend_from_slice(range);
    }
    fn append_string_single(&mut self, character: u8) {
        self.current_string.push(character);
    }
    fn append_string_codepoint(&mut self, codepoint: char) {unreachable!();}
    fn finalize_string(&mut self) {
        let mut done_string = Vec::new();
        ::std::mem::swap(&mut done_string, &mut self.current_string);

        let string = String::from_utf8(done_string);

        self.stack.push(Json::String(string.unwrap()));
    }

    fn finalize_array(&mut self) {}
    fn finalize_map(&mut self) {}
    fn pop_into_map(&mut self) {
        let value = self.stack.pop().unwrap();
        let key = match self.stack.pop().unwrap() {
            Json::String(string) => string,
            _ => unreachable!(),
        };

        match self.stack.last_mut().unwrap() {
            &mut Json::Object(ref mut obj) => {
                obj.push((key, value));
            }
            _ => unreachable!(),
        }
    }
    fn pop_into_array(&mut self) {
        let value = self.stack.pop().unwrap();

        match self.stack.last_mut().unwrap() {
            &mut Json::Array(ref mut arr) => {
                arr.push(value);
            }
            _ => unreachable!(),
        }
    }
}
