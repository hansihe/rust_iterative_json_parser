use ::PResult;
use ::tokenizer::{ Token, TokenizerState, SS };
use ::sink::Sink;
use ::error::ParseError;
use ::input::Range;
use ::source::Source;

#[derive(Debug, Copy, Clone, PartialEq)]
enum ObjectState {
    // Expect key
    // Expect colon
    // Expect value (ReadValue on stack)
    // Expect comma or end
    Key,
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
impl NumberState {
    fn can_end(self) -> bool {
        match self {
            NumberState::DotExponentEnd => true,
            NumberState::ExponentStartEnd => true,
            _ => false,
        }
    }
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
enum State {
    Root,
    Array,
    Object(ObjectState),
    Number(NumberState),
    String,
    ReadValue,
}

#[derive(Debug)]
pub struct ParserState {
    //tokenizer: TokenizerState,
    stack: Vec<State>,
    number_data: NumberData,
    started: bool,
}

enum Transition {
    PopStack,
    ReadValue,
    PopRedo,
    Nothing,
}

macro_rules! unexpected {
    ($ss:expr) => { Err(ParseError::Unexpected($ss.source.position())) }
}

fn log_token(token: &str) {
    //println!("token: {:?}", token);
}


impl ParserState {

    pub fn new() -> Self {
        ParserState {
            //tokenizer: TokenizerState::new(),
            stack: vec![State::ReadValue],
            number_data: NumberData::default(),
            started: false,
        }
    }

    pub fn token_object_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("object_open");

        match *self.stack.last().unwrap() {
            State::ReadValue |
            State::Root => {
                self.stack.pop().unwrap();
                self.stack.push(State::Object(ObjectState::Key));
                ss.sink.push_map();
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_object_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("object_close");

        match *self.stack.last().unwrap() {
            //State::Object(ObjectState::Key) => {
            //    self.stack.pop().unwrap();
            //    ss.sink.finalize_map();
            //    Ok(())
            //},
            State::Object(ObjectState::CommaEnd) => {
                self.stack.pop().unwrap();
                ss.sink.pop_into_map();
                ss.sink.finalize_map();
                Ok(())
            },
            State::ReadValue => {
                self.stack.pop().unwrap();
                if *self.stack.last().unwrap() != State::Object(ObjectState::Colon) {
                    return unexpected!(ss);
                }
                self.stack.pop().unwrap();
                ss.sink.finalize_array();
                Ok(())
            },
            State::Number(number_state) if number_state.can_end() => {
                self.stack.pop().unwrap();
                ss.sink.push_number(self.number_data.clone());
                match *self.stack.last().unwrap() {
                    State::Object(ObjectState::CommaEnd) => {
                        self.stack.pop().unwrap();
                        ss.sink.pop_into_map();
                        ss.sink.finalize_map();
                        Ok(())
                    },
                    _ => unexpected!(ss),
                }
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_array_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("array_open");

        if *self.stack.last().unwrap() != State::ReadValue {
            return unexpected!(ss);
        }

        self.stack.pop().unwrap();
        self.stack.push(State::Array);
        self.stack.push(State::ReadValue);
        ss.sink.push_array();
        Ok(())
    }

    pub fn token_array_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("array_close");

        match *self.stack.last().unwrap() {
            State::Array => {
                self.stack.pop().unwrap();
                ss.sink.pop_into_array();
                ss.sink.finalize_array();
                Ok(())
            },
            State::ReadValue => {
                self.stack.pop().unwrap();
                if *self.stack.last().unwrap() != State::Array {
                    return unexpected!(ss);
                }
                self.stack.pop().unwrap();
                ss.sink.finalize_array();
                Ok(())
            },
            State::Number(number_state) if number_state.can_end() => {
                self.stack.pop().unwrap();
                ss.sink.push_number(self.number_data.clone());
                match *self.stack.last().unwrap() {
                    State::Array => {
                        self.stack.pop().unwrap();
                        ss.sink.pop_into_array();
                        ss.sink.finalize_array();
                        Ok(())
                    },
                    _ => unexpected!(ss),
                }
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_comma<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("comma");

        enum Return {
            None,
            PopNumber(NumberData),
            ReadValue,
        }

        let r = match *self.stack.last_mut().unwrap() {
            State::Array => {
                ss.sink.pop_into_array();
                Return::ReadValue
            },
            State::Object(ref mut state @ ObjectState::CommaEnd) => {
                *state = ObjectState::Key;
                ss.sink.pop_into_map();
                Return::None
            },
            State::Number(number_state) if number_state.can_end() => {
                Return::PopNumber(self.number_data.clone())
            },
            _ => return unexpected!(ss),
        };

        match r {
            Return::None => Ok(()),
            Return::PopNumber(data) => {
                self.stack.pop().unwrap();
                // TODO
                ss.sink.push_number(data);
                match *self.stack.last().unwrap() {
                    State::Array => {
                        self.stack.push(State::ReadValue);
                        ss.sink.pop_into_array();
                        Ok(())
                    },
                    State::Object(ObjectState::CommaEnd) => {
                        // TODO
                        self.stack.pop().unwrap();
                        self.stack.push(State::Object(ObjectState::Key));
                        ss.sink.pop_into_map();
                        Ok(())
                    },
                    _ => unexpected!(ss),
                }
            },
            Return::ReadValue => {
                self.stack.push(State::ReadValue);
                Ok(())
            }
        }
    }

    pub fn token_colon<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("colon");

        match *self.stack.last_mut().unwrap() {
            State::Object(ref mut state @ ObjectState::Colon) => {
                *state = ObjectState::CommaEnd;
            },
            _ => return unexpected!(ss),
        }

        self.stack.push(State::ReadValue);
        Ok(())
    }

    pub fn token_exponent<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("exponent");

        match *self.stack.last_mut().unwrap() {
            State::Number(ref mut state @ NumberState::DotExponentEnd) => {
                *state = NumberState::ExponentSign;
                Ok(())
            },
            State::Number(ref mut state @ NumberState::ExponentStartEnd) => {
                *state = NumberState::ExponentSign;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_dot<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("dot");

        match *self.stack.last_mut().unwrap() {
            State::Number(ref mut state @ NumberState::DotExponentEnd) => {
                *state = NumberState::Decimal;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_sign<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, sign: bool) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("sign");

        enum Return {
            None,
            Push,
        }

        let r = match *self.stack.last_mut().unwrap() {
            State::Number(ref mut state @ NumberState::ExponentSign) => {
                *state = NumberState::Exponent;
                self.number_data.exponent_sign = sign;
                Return::None
            },
            State::ReadValue => {
                self.number_data.sign = sign;
                Return::Push
            },
            _ => return unexpected!(ss),
        };

        match r {
            Return::None => (),
            Return::Push => {
                self.stack.pop().unwrap();
                self.stack.push(State::Number(NumberState::Integer))
            },
        }

        Ok(())
    }

    pub fn token_number<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, range: Range) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("number");

        enum Return {
            None,
            PopStack,
            Push,
        }

        // TODO: Number start
        let r = match *self.stack.last_mut().unwrap() {
            State::Number(ref mut state @ NumberState::Integer) => {
                *state = NumberState::DotExponentEnd;
                self.number_data.integer = range;
                Return::None
            },
            State::Number(ref mut state @ NumberState::Decimal) => {
                *state = NumberState::ExponentStartEnd;
                self.number_data.decimal = Some(range);
                Return::None
            },
            State::Number(NumberState::Exponent) => {
                Return::PopStack
            },
            State::ReadValue => {
                Return::Push
            },
            _ => return unexpected!(ss),
        };

        match r {
            Return::None => (),
            Return::PopStack => {
                self.stack.pop().unwrap();
                ()
            }
            Return::Push => {
                self.number_data.integer = range;
                self.stack.pop().unwrap();
                self.stack.push(State::Number(NumberState::DotExponentEnd));
                ()
            }
        }

        Ok(())
    }

    pub fn token_bool<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, value: bool) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        if *self.stack.last().unwrap() != State::ReadValue {
            return unexpected!(ss);
        }
        self.stack.pop().unwrap();
        ss.sink.push_bool(value);
        Ok(())
    }

    pub fn token_null<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        if *self.stack.last().unwrap() != State::ReadValue {
            return unexpected!(ss);
        }
        self.stack.pop().unwrap();
        ss.sink.push_null();
        Ok(())
    }

    pub fn token_quote<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("quote");

        enum Return {
            StartString,
            StartStringReplace,
            EndString,
        }

        let r = match *self.stack.last_mut().unwrap() {
            State::String => Return::EndString,
            State::Object(ref mut state @ ObjectState::Key) => {
                *state = ObjectState::Colon;
                Return::StartString
            },
            State::ReadValue => Return::StartStringReplace,
            _ => return unexpected!(ss),
        };

        match r {
            Return::StartString => {
                self.stack.push(State::String);
                ss.sink.start_string();
            },
            Return::StartStringReplace => {
                self.stack.pop().unwrap();
                self.stack.push(State::String);
                ss.sink.start_string();
            },
            Return::EndString => {
                self.stack.pop();
                ss.sink.finalize_string();
            },
        }

        Ok(())
    }

    // Tokenizer guarantees that these are only called between token_quotes.
    pub fn token_string_range<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, range: Range) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("string_range");

        ss.sink.append_string_range(range);
        Ok(())
    }
    pub fn token_string_single<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, byte: u8) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("string_single");

        ss.sink.append_string_single(byte);
        Ok(())
    }
    pub fn token_string_codepoint<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, codepoint: char) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("string_codepoint");

        ss.sink.append_string_codepoint(codepoint);
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.stack.len() == 0
    }

}
