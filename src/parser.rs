use ::PResult;
use ::tokenizer::{ Token, TokenizerState, SS };
use ::sink::Sink;
use ::error::ParseError;
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
enum StackState {
    Array,
    Object(ObjectState),
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum TopState {
    None,
    ReadValue,
    String,
    Number(NumberState),
}

#[derive(Debug)]
pub struct ParserState {
    stack: Vec<StackState>,
    state: TopState,
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
macro_rules! handle_bail {
    ($sink_expr:expr) => { handle_bail!($sink_expr, {}) };
    ($sink_expr:expr, $exit_plan:expr) => {
        match $sink_expr {
            Ok(()) => {},
            Err(bail) => {
                $exit_plan;
                return Err(ParseError::SinkBail(bail));
            },
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
            state: TopState::ReadValue,
            number_data: NumberData::default(),
            started: false,
        }
    }

    pub fn close_number<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        match self.state {
            TopState::Number(NumberState::DotExponentEnd) |
            TopState::Number(NumberState::ExponentStartEnd) => {
                self.state = TopState::None;
                let mut number_data = NumberData::default();
                ::std::mem::swap(&mut number_data, &mut self.number_data);
                // FIXME FIXME FIXME jasdfjasdfhiu;
                handle_bail!(ss.sink.push_number(number_data));
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_object_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("object_open");

        if self.state != TopState::ReadValue {
            return unexpected!(ss);
        }

        self.state = TopState::None;
        self.stack.push(StackState::Object(ObjectState::KeyEnd));
        ss.sink.push_map();

        Ok(())
    }

    pub fn token_object_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("object_close");

        match self.state {
            TopState::None => {},
            TopState::ReadValue => {
                self.state = TopState::None;
            },
            TopState::Number(_) => self.close_number(ss)?,
            _ => return unexpected!(ss),
        }

        match *self.stack.last().unwrap() {
            StackState::Object(ObjectState::KeyEnd) => {
                self.stack.pop().unwrap();
                ss.sink.finalize_map();
                Ok(())
            },
            StackState::Object(ObjectState::CommaEnd) => {
                self.stack.pop().unwrap();
                ss.sink.pop_into_map();
                ss.sink.finalize_map();
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_array_open<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("array_open");

        if self.state != TopState::ReadValue {
            return unexpected!(ss);
        }

        self.state = TopState::ReadValue;
        self.stack.push(StackState::Array);
        ss.sink.push_array();

        Ok(())
    }

    pub fn token_array_close<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("array_close");

        match self.state {
            TopState::None => {
                ss.sink.pop_into_array();
            },
            TopState::ReadValue => {
                self.state = TopState::None;
            },
            TopState::Number(_) => {
                self.close_number(ss)?;
                ss.sink.pop_into_array();
            },
            _ => return unexpected!(ss),
        }

        match *self.stack.last().unwrap() {
            StackState::Array => {
                self.stack.pop().unwrap();
                ss.sink.finalize_array();
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_comma<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("comma");

        match self.state {
            TopState::None => {},
            TopState::Number(_) => self.close_number(ss)?,
            _ => return unexpected!(ss)
        }

        match *self.stack.last_mut().unwrap() {
            StackState::Array => {
                self.state = TopState::ReadValue;
                ss.sink.pop_into_array();
                Ok(())
            },
            StackState::Object(ref mut state @ ObjectState::CommaEnd) => {
                *state = ObjectState::Key;
                ss.sink.pop_into_map();
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_colon<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("colon");

        match *self.stack.last_mut().unwrap() {
            StackState::Object(ref mut state @ ObjectState::Colon) => {
                self.state = TopState::ReadValue;
                *state = ObjectState::CommaEnd;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_exponent<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("exponent");

        match self.state {
            TopState::Number(ref mut state @ NumberState::DotExponentEnd) => {
                *state = NumberState::ExponentSign;
                Ok(())
            },
            TopState::Number(ref mut state @ NumberState::ExponentStartEnd) => {
                *state = NumberState::ExponentSign;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_dot<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("dot");

        match self.state {
            TopState::Number(ref mut state @ NumberState::DotExponentEnd) => {
                *state = NumberState::Decimal;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_sign<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, sign: bool) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("sign");

        match self.state {
            TopState::ReadValue => {
                self.state = TopState::Number(NumberState::Integer);
                self.number_data = NumberData::default();
                self.number_data.sign = sign;
                Ok(())
            },
            TopState::Number(ref mut state @ NumberState::ExponentSign) => {
                *state = NumberState::Exponent;
                self.number_data.exponent_sign = sign;
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_number<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, range: Range) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("number");

        match self.state {
            TopState::ReadValue |
            TopState::Number(NumberState::Integer) => {
                self.state = TopState::Number(NumberState::DotExponentEnd);
                self.number_data.integer = range;
                Ok(())
            },
            TopState::Number(ref mut state @ NumberState::Decimal) => {
                *state = NumberState::ExponentStartEnd;
                self.number_data.decimal = Some(range);
                Ok(())
            },
            TopState::Number(NumberState::ExponentSign) |
            TopState::Number(NumberState::Exponent) => {
                self.state = TopState::None;
                self.number_data.exponent = Some(range);
                let mut number_data = NumberData::default();
                ::std::mem::swap(&mut number_data, &mut self.number_data);
                handle_bail!(ss.sink.push_number(number_data));
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_bool<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, value: bool) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("bool");

        match self.state {
            TopState::ReadValue => {
                self.state = TopState::None;
                ss.sink.push_bool(value);
                Ok(())
            },
            _ => unexpected!(ss),
        }
    }

    pub fn token_null<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("null");

        match self.state {
            TopState::ReadValue => {
                self.state = TopState::None;
                ss.sink.push_null();
                Ok(())
            }
            _ => unexpected!(ss),
        }
    }

    pub fn token_quote<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        log_token("quote");

        match self.state {
            TopState::None => {
                match *self.stack.last_mut().unwrap() {
                    StackState::Object(ref mut state @ ObjectState::Key) |
                    StackState::Object(ref mut state @ ObjectState::KeyEnd) => {
                        *state = ObjectState::Colon;
                        self.state = TopState::String;
                        ss.sink.start_string();
                        Ok(())
                    },
                    _ => unexpected!(ss),
                }
            },
            TopState::ReadValue => {
                self.state = TopState::String;
                ss.sink.start_string();
                Ok(())
            },
            TopState::String => {
                self.state = TopState::None;
                ss.sink.finalize_string();
                Ok(())
            },
            _ => unexpected!(ss),
        }
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
