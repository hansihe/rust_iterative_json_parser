use super::ParserSink;

#[derive(Debug)]
pub enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

impl Json {
    fn extract_string(self) -> String {
        match self {
            Json::String(string) => string,
            _ => panic!("expected string"),
        }
    }
}

pub struct EnumSink {
    stack: Vec<Json>,
}

impl EnumSink {
    pub fn new() -> EnumSink {
        EnumSink {
            stack: vec![],
        }
    }

    pub fn to_result(mut self) -> Json {
        if self.stack.len() != 1 {
            panic!("Result not ready.");
        }
        self.stack.pop().unwrap()
    }
}

impl ParserSink for EnumSink {
    fn push_map(&mut self) { self.stack.push(Json::Object(vec![])) }
    fn push_array(&mut self) { self.stack.push(Json::Array(vec![])) }
    fn push_string(&mut self, string: &str) { self.stack.push(Json::String(string.to_string())) }
    fn push_number(&mut self, number: f64) { self.stack.push(Json::Number(number)) }
    fn push_bool(&mut self, boolean: bool) { self.stack.push(Json::Boolean(boolean)) }
    fn push_null(&mut self) { self.stack.push(Json::Null) }
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
