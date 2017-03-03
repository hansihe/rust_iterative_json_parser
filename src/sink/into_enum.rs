use ::Bailable;
use super::{Sink, NumberData, Position, StringPosition};
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
pub struct EnumSink<'a> {
    pub stack: Vec<Json>,
    source: &'a [u8],
    current_string: Vec<u8>,
    bail: bool,
}

impl<'a> EnumSink<'a> {
    pub fn new(source: &'a [u8]) -> EnumSink {
        EnumSink {
            stack: vec![],
            source: source,
            current_string: Vec::new(),
            bail: false,
        }
    }
    pub fn new_bailing(source: &'a [u8]) -> EnumSink {
        let mut sink = EnumSink::new(source);
        sink.bail = true;
        sink
    }

    fn range_to_str<'b>(&'b mut self, range: Range) -> &'b str {
        let raw = &self.source[(range.start)..(range.end)];
        ::std::str::from_utf8(raw).unwrap()
    }

    pub fn to_result(mut self) -> Json {
        // println!("to_result: {:?}", self);
        if self.stack.len() != 1 {
            panic!("Result not ready.");
        }
        self.stack.pop().unwrap()
    }
}

impl<'a> Bailable for EnumSink<'a> {
    type Bail = ();
}

impl<'a> Sink for EnumSink<'a> {
    fn push_map(&mut self, pos: Position) {
        self.stack.push(Json::Object(vec![]));
        if self.stack.len() == 1 {
            assert_eq!(pos, Position::Root);
            return;
        }
        match self.stack[self.stack.len() - 2] {
            Json::String(_) => assert_eq!(pos, Position::MapValue),
            Json::Array(_) => assert_eq!(pos, Position::ArrayValue),
            _ => panic!(),
        }
    }
    fn push_array(&mut self, pos: Position) {
        self.stack.push(Json::Array(vec![]));
        if self.stack.len() == 1 {
            assert_eq!(pos, Position::Root);
            return;
        }
        match self.stack[self.stack.len() - 2] {
            Json::String(_) => assert_eq!(pos, Position::MapValue),
            Json::Array(_) => assert_eq!(pos, Position::ArrayValue),
            _ => panic!(),
        }
    }
    fn push_number(&mut self, _pos: Position, number: NumberData) -> Result<(), Self::Bail> {
        let mut out = String::new();

        if number.sign {
            out.push('+');
        } else {
            out.push('-');
        }

        out.push_str(self.range_to_str(number.integer));

        out.push('.');

        if let Some(range) = number.decimal {
            out.push_str(self.range_to_str(range));
        } else {
            out.push('0');
        }

        out.push('e');

        if number.exponent_sign {
            out.push('+');
        } else {
            out.push('-');
        }

        if let Some(range) = number.exponent {
            out.push_str(self.range_to_str(range));
        } else {
            out.push('1')
        }

        self.stack.push(Json::Number(out));

        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }
    fn push_bool(&mut self, _pos: Position, boolean: bool) -> Result<(), Self::Bail> {
        self.stack.push(Json::Boolean(boolean));
        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }
    fn push_null(&mut self, _pos: Position) -> Result<(), Self::Bail> {
        self.stack.push(Json::Null);
        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }

    fn start_string(&mut self, _pos: StringPosition) {}
    fn append_string_range(&mut self, string: Range) {
        let range = &self.source[(string.start)..(string.end)];
        self.current_string.extend_from_slice(range);
    }
    fn append_string_single(&mut self, character: u8) {
        self.current_string.push(character);
    }
    fn append_string_codepoint(&mut self, codepoint: char) {
        let mut buf: [u8; 4] = [0, 0, 0, 0];
        let codepoint_slice = codepoint.encode_utf8(&mut buf);
        self.current_string.extend_from_slice(codepoint_slice.as_bytes());
    }
    fn finalize_string(&mut self, _pos: StringPosition) -> Result<(), Self::Bail> {
        let mut done_string = Vec::new();
        ::std::mem::swap(&mut done_string, &mut self.current_string);

        let string = String::from_utf8(done_string);

        self.stack.push(Json::String(string.unwrap()));

        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }

    fn finalize_array(&mut self, pos: Position) -> Result<(), Self::Bail> {
        if self.stack.len() == 1 {
            assert_eq!(pos, Position::Root);
            return if self.bail {
                Err(())
            } else {
                Ok(())
            };
        }
        match self.stack[self.stack.len() - 2] {
            Json::String(_) => assert_eq!(pos, Position::MapValue),
            Json::Array(_) => assert_eq!(pos, Position::ArrayValue),
            _ => panic!(),
        }
        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }
    fn finalize_map(&mut self, pos: Position) -> Result<(), Self::Bail> {
        if self.stack.len() == 1 {
            assert_eq!(pos, Position::Root);
            return if self.bail {
                Err(())
            } else {
                Ok(())
            };
        }
        match self.stack[self.stack.len() - 2] {
            Json::String(_) => assert_eq!(pos, Position::MapValue),
            Json::Array(_) => assert_eq!(pos, Position::ArrayValue),
            _ => panic!(),
        }
        if self.bail {
            Err(())
        } else {
            Ok(())
        }
    }
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
