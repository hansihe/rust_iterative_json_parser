use ::PResult;
use ::tokenizer::{ SS };
use ::sink::Sink;
use ::error::{ParseError, Unexpected};
use ::input::Range;
use ::source::Source;

#[derive(Debug, Copy, Clone, PartialEq)]
enum ObjectState {
    Key,
    KeyEnd,
    Colon,
    CommaEnd,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum NumberState {
    Integer, // Integer
    DotExponentEnd, // '.' or end
    Decimal, // Decimal
    ExponentStartEnd, // 'eE' or end
    ExponentSign, // '-+' or Exponent
    Exponent, // Exponent then end
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberData {
    pub sign: bool,
    pub integer: Range,
    pub decimal: Option<Range>,
    pub exponent_sign: bool,
    pub exponent: Option<Range>,
}
impl Default for NumberData {
    fn default() -> Self {
        NumberData {
            sign: true,
            integer: Range::new(0.into(), 0.into()),
            decimal: None,
            exponent_sign: true,
            exponent: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum StackState {
    Array,
    Object,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum TopState {
    None,

    ArrayCommaEnd,

    ObjectKeyEnd,
    ObjectColon,
    ObjectCommaEnd,

    Number(TopStateContext),
    String(TopStateContext),
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum TopStateContext {
    None,
    ObjectKey,
    ObjectValue,
    ArrayValue,
}
impl TopStateContext {
    fn from_topstate(state: TopState) -> TopStateContext {
        match state {
            TopState::None => TopStateContext::None,
            TopState::ObjectKeyEnd => TopStateContext::ObjectKey,
            TopState::ObjectCommaEnd => TopStateContext::ObjectValue,
            TopState::ArrayCommaEnd => TopStateContext::ArrayValue,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct ParserState {
    stack: Vec<StackState>,

    state: TopState,
    read_value: bool,

    number_state: NumberState,
    number_data: NumberData,
}

macro_rules! unexpected {
    ($ss:expr, $reason:expr) => { Err(ParseError::Unexpected($ss.source.position(), $reason)) }
}

macro_rules! matches {
    ($e:expr, $p:pat) => {
        match $e {
            $p => true,
            _ => false,
        }
    };
}

fn log_token(token: &str) {
    //println!("token: {:?}", token);
}


impl ParserState {

    pub fn new() -> Self {
        ParserState {
            stack: vec![],

            state: TopState::None,
            read_value: true,

            number_state: NumberState::Integer,
            number_data: NumberData::default(),
        }
    }

    fn handle_end_number<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, next: TopStateContext) -> bool where Src: Source, Snk: Sink {
        if self.number_state != NumberState::ExponentStartEnd
            && self.number_state != NumberState::DotExponentEnd {
                return false;
            }

        ss.sink.push_number(self.number_data.clone());

        self.state = match next {
            TopStateContext::None => TopState::None,
            TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
            TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
            TopStateContext::ObjectKey => unreachable!(),
        };

        true
    }

    pub fn token_object_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("object_open");

        if !self.read_value {
            return unexpected!(ss, Unexpected::ObjectOpen);
        }
        self.read_value = false;

        ss.sink.push_map();
        self.stack.push(StackState::Object);
        self.state = TopState::ObjectKeyEnd;
        Ok(())
    }

    pub fn token_array_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("array_open");

        if !self.read_value {
            return unexpected!(ss, Unexpected::ArrayOpen);
        }
        self.read_value = true;

        ss.sink.push_array();
        self.stack.push(StackState::Array);
        self.state = TopState::ArrayCommaEnd;
        Ok(())
    }

    pub fn token_object_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("object_close");

        match self.state {
            // An object end can only occur if we are waiting for a comma or waiting
            // for a key. We are diverging a bit from the spec here, and are allowing
            // trailing commas. This makes the state machine a bit simpler, and I like
            // trailing commas,
            TopState::ObjectKeyEnd | TopState::ObjectCommaEnd | TopState::Number(TopStateContext::ObjectValue) => {
                if let TopState::Number(context) = self.state {
                    if !self.handle_end_number(ss, context) {
                        return unexpected!(ss, Unexpected::ObjectClose);
                    }
                }

                // If the read_value flag is not set, it means we just read in a value
                // and need to pop_into_map.
                if !self.read_value && self.state == TopState::ObjectCommaEnd {
                    ss.sink.pop_into_map();
                }

                if self.read_value && self.state == TopState::ObjectCommaEnd {
                    return unexpected!(ss, Unexpected::ObjectClose);
                }

                self.read_value = false;
                ss.sink.finalize_map();

                // Because it is impossible to end up in a TopState::Object* without
                // the top value on the stack being an StackState::Object, we can be
                // sure that there is a value for us to pop, and that it, in fact, is
                // a StackState::Object.
                self.stack.pop().unwrap();

                // Look at the last value on the stack to determine what our next state
                // should be.
                self.state = match self.stack.last() {
                    Some(&StackState::Object) => TopState::ObjectCommaEnd,
                    Some(&StackState::Array) => TopState::ArrayCommaEnd,
                    None => TopState::None,
                };
            },
            _ => return unexpected!(ss, Unexpected::ObjectClose),
        }

        Ok(())
    }

    pub fn token_array_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("array_close");

        match self.state {
            TopState::ArrayCommaEnd | TopState::Number(TopStateContext::ArrayValue) => {
                if let TopState::Number(context) = self.state {
                    if !self.handle_end_number(ss, context) {
                        return unexpected!(ss, Unexpected::ObjectClose);
                    }
                }

                if !self.read_value {
                    ss.sink.pop_into_array();
                }
                self.read_value = false;

                ss.sink.finalize_array();

                self.stack.pop().unwrap();

                self.state = match self.stack.last() {
                    Some(&StackState::Object) => TopState::ObjectCommaEnd,
                    Some(&StackState::Array) => TopState::ArrayCommaEnd,
                    None => TopState::None,
                };
            },
            _ => return unexpected!(ss, Unexpected::ObjectClose),
        }

        Ok(())
    }

    pub fn token_comma<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("comma");

        match self.state {
            TopState::ObjectCommaEnd if !self.read_value => {
                ss.sink.pop_into_map();
                self.state = TopState::ObjectKeyEnd;
            },
            TopState::ArrayCommaEnd if !self.read_value => {
                ss.sink.pop_into_array();
                self.read_value = true;
            },
            TopState::Number(context) => {
                if !self.handle_end_number(ss, context) {
                    return unexpected!(ss, Unexpected::Comma);
                }
                match self.state {
                    TopState::ObjectCommaEnd => {
                        ss.sink.pop_into_map();
                        self.state = TopState::ObjectKeyEnd;
                    },
                    TopState::ArrayCommaEnd => {
                        ss.sink.pop_into_array();
                        self.read_value = true;
                    },
                    _ => return unexpected!(ss, Unexpected::Comma),
                }
            },
            _ => return unexpected!(ss, Unexpected::Comma),
        }

        Ok(())
    }

    pub fn token_colon<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("colon");

        match self.state {
            TopState::ObjectColon => {
                self.state = TopState::ObjectCommaEnd;
                self.read_value = true;
            },
            _ => return unexpected!(ss, Unexpected::Colon),
        }

        Ok(())
    }

    pub fn token_exponent<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("exponent");

        if !matches!(self.state, TopState::Number(_)) {
            return unexpected!(ss, Unexpected::Exponent);
        }

        self.number_state = match self.number_state {
            NumberState::DotExponentEnd => NumberState::ExponentSign,
            NumberState::ExponentStartEnd => NumberState::ExponentSign,
            _ => return unexpected!(ss, Unexpected::Exponent),
        };

        Ok(())
    }

    pub fn token_dot<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("dot");

        if !matches!(self.state, TopState::Number(_))
            || self.number_state != NumberState::DotExponentEnd {
                return unexpected!(ss, Unexpected::Dot);
            }
        self.number_state = NumberState::Decimal;

        Ok(())
    }

    pub fn token_sign<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, sign: bool) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("sign");

        match self.state {
            TopState::Number(_) => {
                if self.number_state != NumberState::ExponentSign {
                    return unexpected!(ss, Unexpected::Sign);
                }
                self.number_data.exponent_sign = sign;
                self.number_state = NumberState::Exponent;
            },
            _ => {
                if !self.read_value {
                    return unexpected!(ss, Unexpected::Sign);
                }
                self.read_value = false;
                self.number_data = NumberData::default();
                self.number_data.sign = sign;
                self.state = TopState::Number(TopStateContext::from_topstate(self.state));
                self.number_state = NumberState::Integer;
            },
        }

        Ok(())
    }

    pub fn token_number<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, range: Range) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("number");

        match self.state {
            TopState::Number(context) => {
                match self.number_state {
                    NumberState::Integer => {
                        self.number_data.integer = range;
                        self.number_state = NumberState::DotExponentEnd;
                    },
                    NumberState::Decimal => {
                        self.number_data.decimal = Some(range);
                        self.number_state = NumberState::ExponentStartEnd;
                    },
                    NumberState::ExponentSign | NumberState::Exponent => {
                        self.number_data.exponent = Some(range);
                        ss.sink.push_number(self.number_data.clone());
                        self.state = match context {
                            TopStateContext::None => TopState::None,
                            TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
                            TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
                            TopStateContext::ObjectKey => unreachable!(),
                        };
                    },
                    _ => return unexpected!(ss, Unexpected::Number),
                }
            },
            _ => {
                if !self.read_value {
                    return unexpected!(ss, Unexpected::Number);
                }
                self.read_value = false;
                self.number_data = NumberData::default();
                self.number_data.integer = range;
                self.number_state = NumberState::DotExponentEnd;
                self.state = TopState::Number(TopStateContext::from_topstate(self.state));
            },
        }

        Ok(())
    }

    pub fn token_bool<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, value: bool) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("bool");

        if !self.read_value {
            return unexpected!(ss, Unexpected::Bool);
        }
        self.read_value = false;

        ss.sink.push_bool(value);

        Ok(())
    }

    pub fn token_null<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("null");

        if !self.read_value {
            return unexpected!(ss, Unexpected::Bool);
        }
        self.read_value = false;

        ss.sink.push_null();

        Ok(())
    }

    pub fn token_quote<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("quote");

        match self.state {
            TopState::String(context) => {
                self.state = match context {
                    TopStateContext::None => TopState::None,
                    TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
                    TopStateContext::ObjectKey => TopState::ObjectColon,
                    TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
                };
                ss.sink.finalize_string();
            },
            _ => {
                if !self.read_value && !(self.state == TopState::ObjectKeyEnd) {
                    return unexpected!(ss, Unexpected::Quote);
                }

                self.read_value = false;
                self.state = TopState::String(TopStateContext::from_topstate(self.state));
                ss.sink.start_string();
            },
        }

        Ok(())
    }

    // Tokenizer guarantees that these are only called between token_quotes.
    pub fn token_string_range<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, range: Range) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("string_range");

        ss.sink.append_string_range(range);
        Ok(())
    }
    pub fn token_string_single<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, byte: u8) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("string_single");

        ss.sink.append_string_single(byte);
        Ok(())
    }
    pub fn token_string_codepoint<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, codepoint: char) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        log_token("string_codepoint");

        ss.sink.append_string_codepoint(codepoint);
        Ok(())
    }

    pub fn finish<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) where Src: Source, Snk: Sink {
        if self.state == TopState::Number(TopStateContext::None) {
            self.handle_end_number(ss, TopStateContext::None);
        }
    }

    pub fn finished(&self) -> bool {
        self.state == TopState::None && self.stack.len() == 0 && !self.read_value
    }

}
