use ::PResult;
use ::Bailable;
use ::sink::{Sink, Position, StringPosition};
use ::error::{ParseError, Unexpected};
use ::input::Range;
use ::source::Source;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum NumberState {
    Integer, // Integer
    DotExponentEnd, // '.' or end
    Decimal, // Decimal
    ExponentStartEnd, // 'eE' or end
    ExponentSign, // '-+' or Exponent
    Exponent, // Exponent then end
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ReentryAction {
    None,
    FinishObjectClose,
    FinishArrayClose,
    FinishNumberComma,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum StackState {
    Array,
    Object,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TopState {
    None,

    ArrayCommaEnd,

    ObjectKeyEnd,
    ObjectColon,
    ObjectCommaEnd,

    Number(TopStateContext),
    String(TopStateContext),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    fn string_position(&self) -> StringPosition {
        match *self {
            TopStateContext::ObjectKey => StringPosition::MapKey,
            TopStateContext::ObjectValue => StringPosition::MapValue,
            TopStateContext::ArrayValue => StringPosition::ArrayValue,
            TopStateContext::None => StringPosition::Root,
        }
    }
}

#[derive(Debug)]
pub struct ParserState {
    stack: Vec<StackState>,

    state: TopState,
    read_value: bool,
    reentry_action: ReentryAction,
    started: bool,

    number_state: NumberState,
    number_data: NumberData,
}

macro_rules! unexpected {
    ($ss:expr, $reason:expr) => { Err(ParseError::Unexpected($ss.position(), $reason)) }
}

macro_rules! lift_bail {
    ($bailing:expr) => {
        match $bailing {
            Ok(inner) => Ok(inner),
            Err(bail) => Err(ParseError::SourceBail(bail)),
        }
    };
}
macro_rules! lift_bail_sink {
    ($bailing:expr) => {
        match $bailing {
            Ok(inner) => Ok(inner),
            Err(bail) => Err(ParseError::SourceBail(bail)),
        }
    };
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
            read_value: false,
            reentry_action: ReentryAction::None,
            started: false,

            number_state: NumberState::Integer,
            number_data: NumberData::default(),
        }
    }

    fn get_position(&self) -> Position {
        match self.stack.last() {
            None => Position::Root,
            Some(&StackState::Object) => Position::MapValue,
            Some(&StackState::Array) => Position::ArrayValue,
        }
    }

    fn handle_end_number<SS>(&mut self, ss: &mut SS, position: Position, next: TopStateContext, unexpected: Unexpected) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        if self.number_state != NumberState::ExponentStartEnd
            && self.number_state != NumberState::DotExponentEnd {
                return unexpected!(ss, unexpected);
            }

        self.state = match next {
            TopStateContext::None => {
                self.state = TopState::None;
                lift_bail_sink!(ss.push_number(position, self.number_data.clone()))?;
                return Err(ParseError::End);
            },
            TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
            TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
            TopStateContext::ObjectKey => unreachable!(),
        };

        lift_bail_sink!(ss.push_number(position, self.number_data.clone()))?;

        Ok(())
    }

    pub fn token_object_open<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("object_open");

        if !self.read_value && self.state != TopState::None {
            return unexpected!(ss, Unexpected::ObjectOpen);
        }
        self.read_value = false;
        self.started = true;

        ss.push_map(self.get_position());
        self.stack.push(StackState::Object);
        self.state = TopState::ObjectKeyEnd;
        Ok(())
    }

    pub fn token_array_open<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("array_open");

        if !self.read_value && self.state != TopState::None {
            return unexpected!(ss, Unexpected::ArrayOpen);
        }
        self.read_value = true;
        self.started = true;

        ss.push_array(self.get_position());
        self.stack.push(StackState::Array);
        self.state = TopState::ArrayCommaEnd;
        Ok(())
    }

    pub fn token_object_close<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("object_close");

        match self.state {
            // An object end can only occur if we are waiting for a comma or waiting
            // for a key. We are diverging a bit from the spec here, and are allowing
            // trailing commas. This makes the state machine a bit simpler, and I like
            // trailing commas,
            TopState::ObjectKeyEnd | TopState::ObjectCommaEnd | TopState::Number(TopStateContext::ObjectValue) => {
                if let TopState::Number(context) = self.state {
                    match self.handle_end_number(ss, Position::MapValue, context, Unexpected::ObjectClose) {
                        Ok(()) => (),
                        Err(err) => {
                            self.reentry_action = ReentryAction::FinishObjectClose;
                            return Err(err);
                        },
                    }
                }

                self.finish_object_close(ss)?;
            },
            _ => return unexpected!(ss, Unexpected::ObjectClose),
        }

        Ok(())
    }

    pub fn finish_object_close<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        if self.read_value && self.state == TopState::ObjectCommaEnd {
            return unexpected!(ss, Unexpected::ObjectClose);
        }

        // If the read_value flag is not set, it means we just read in a value
        // and need to pop_into_map.
        if !self.read_value && self.state == TopState::ObjectCommaEnd {
            ss.pop_into_map();
        }

        self.read_value = false;

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

        lift_bail_sink!(ss.finalize_map(self.get_position()))?;
        if self.stack.len() == 0 {
            return Err(ParseError::End);
        }

        Ok(())
    }

    pub fn token_array_close<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("array_close");

        match self.state {
            TopState::ArrayCommaEnd | TopState::Number(TopStateContext::ArrayValue) => {
                if let TopState::Number(context) = self.state {
                    match self.handle_end_number(ss, Position::ArrayValue, context, Unexpected::ObjectClose) {
                        Ok(()) => (),
                        Err(err) => {
                            self.reentry_action = ReentryAction::FinishArrayClose;
                            return Err(err);
                        },
                    }
                }
                self.finish_array_close(ss)?;
            },
            _ => return unexpected!(ss, Unexpected::ObjectClose),
        }

        Ok(())
    }

    pub fn finish_array_close<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        if !self.read_value {
            ss.pop_into_array();
        }

        self.read_value = false;

        self.stack.pop().unwrap();

        self.state = match self.stack.last() {
            Some(&StackState::Object) => TopState::ObjectCommaEnd,
            Some(&StackState::Array) => TopState::ArrayCommaEnd,
            None => TopState::None,
        };

        lift_bail_sink!(ss.finalize_array(self.get_position()))?;
        if self.stack.len() == 0 {
            return Err(ParseError::End);
        }

        Ok(())
    }

    pub fn token_comma<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("comma");

        match self.state {
            TopState::ObjectCommaEnd if !self.read_value => {
                self.state = TopState::ObjectKeyEnd;
                ss.pop_into_map();
            },
            TopState::ArrayCommaEnd if !self.read_value => {
                self.read_value = true;
                ss.pop_into_array();
            },
            TopState::Number(context) => {
                let position = self.get_position();
                match self.handle_end_number(ss, position, context, Unexpected::Comma) {
                    Ok(()) => (),
                    Err(err) => {
                        self.reentry_action = ReentryAction::FinishNumberComma;
                        return Err(err);
                    },
                }
                self.finish_number_token_comma(ss)?;
            },
            _ => return unexpected!(ss, Unexpected::Comma),
        }

        Ok(())
    }

    pub fn finish_number_token_comma<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        match self.state {
            TopState::ObjectCommaEnd => {
                self.state = TopState::ObjectKeyEnd;
                ss.pop_into_map();
            },
            TopState::ArrayCommaEnd => {
                self.read_value = true;
                ss.pop_into_array();
            },
            _ => return unexpected!(ss, Unexpected::Comma),
        }
        Ok(())
    }

    pub fn token_colon<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
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

    pub fn token_exponent<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
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

    pub fn token_dot<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("dot");

        if !matches!(self.state, TopState::Number(_))
            || self.number_state != NumberState::DotExponentEnd {
                return unexpected!(ss, Unexpected::Dot);
            }
        self.number_state = NumberState::Decimal;

        Ok(())
    }

    pub fn token_sign<SS>(&mut self, ss: &mut SS, sign: bool) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
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

    pub fn token_number<SS>(&mut self, ss: &mut SS, range: Range) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
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
                        self.state = match context {
                            TopStateContext::None => {
                                lift_bail_sink!(ss.push_number(self.get_position(), self.number_data.clone()))?;
                                return Err(ParseError::End);
                            }
                            TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
                            TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
                            TopStateContext::ObjectKey => unreachable!(),
                        };
                        lift_bail_sink!(ss.push_number(self.get_position(), self.number_data.clone()))?;
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

    pub fn token_bool<SS>(&mut self, ss: &mut SS, value: bool) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("bool");

        if !self.read_value {
            return unexpected!(ss, Unexpected::Bool);
        }
        self.read_value = false;

        lift_bail_sink!(ss.push_bool(self.get_position(), value))?;
        if self.stack.len() == 0 {
            return Err(ParseError::End);
        }

        Ok(())
    }

    pub fn token_null<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("null");

        if !self.read_value {
            return unexpected!(ss, Unexpected::Bool);
        }
        self.read_value = false;

        lift_bail_sink!(ss.push_null(self.get_position()))?;
        if self.stack.len() == 0 {
            return Err(ParseError::End);
        }

        Ok(())
    }

    pub fn token_quote<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("quote");

        match self.state {
            TopState::String(context) => {
                self.state = match context {
                    TopStateContext::None => {
                        self.state = TopState::None;
                        lift_bail_sink!(ss.finalize_string(context.string_position()))?;
                        return Err(ParseError::End);
                    },
                    TopStateContext::ArrayValue => TopState::ArrayCommaEnd,
                    TopStateContext::ObjectKey => TopState::ObjectColon,
                    TopStateContext::ObjectValue => TopState::ObjectCommaEnd,
                };
                lift_bail_sink!(ss.finalize_string(context.string_position()))?;
            },
            _ => {
                if !self.read_value && !(self.state == TopState::ObjectKeyEnd) {
                    return unexpected!(ss, Unexpected::Quote);
                }

                self.read_value = false;
                let context = TopStateContext::from_topstate(self.state);
                self.state = TopState::String(context);
                ss.start_string(context.string_position());
            },
        }

        Ok(())
    }

    // Tokenizer guarantees that these are only called between token_quotes.
    pub fn token_string_range<SS>(&mut self, ss: &mut SS, range: Range) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("string_range");

        ss.append_string_range(range);
        Ok(())
    }
    pub fn token_string_single<SS>(&mut self, ss: &mut SS, byte: u8) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("string_single");

        ss.append_string_single(byte);
        Ok(())
    }
    pub fn token_string_codepoint<SS>(&mut self, ss: &mut SS, codepoint: char) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        log_token("string_codepoint");

        ss.append_string_codepoint(codepoint);
        Ok(())
    }

    pub fn reentry<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        let action = self.reentry_action;
        self.reentry_action = ReentryAction::None;
        match action {
            ReentryAction::FinishArrayClose => self.finish_array_close(ss)?,
            ReentryAction::FinishObjectClose => self.finish_object_close(ss)?,
            ReentryAction::FinishNumberComma => self.finish_number_token_comma(ss)?,
            ReentryAction::None => (),
        }
        Ok(())
    }

    pub fn finish<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail> where SS: Source + Sink + Bailable {
        if self.state == TopState::Number(TopStateContext::None) {
            self.handle_end_number(ss, Position::Root, TopStateContext::None, Unexpected::Eof)?;
        }
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.state == TopState::None && self.stack.len() == 0 && !self.read_value && self.started
    }

}
