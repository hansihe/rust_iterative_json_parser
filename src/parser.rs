use ::PResult;
use ::tokenizer::{ Token, Tokenizer };
use ::sink::ParserSink;
use ::error::ParseError;
use ::input::Range;
use ::source::Source;

#[derive(Debug)]
enum ObjectState {
    Start,
    Key,
    Colon,
    CommaEnd,
}

#[derive(Debug)]
enum ArrayState {
    Start,
    CommaEnd,
}

#[derive(Debug)]
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
    pub integer: Option<Range>,
    pub decimal: Option<Range>,
    pub exponent_sign: bool,
    pub exponent: Option<Range>,
}
impl Default for NumberData {
    fn default() -> Self {
        NumberData {
            sign: true,
            integer: None,
            decimal: None,
            exponent_sign: true,
            exponent: None,
        }
    }
}

#[derive(Debug)]
enum State {
    Root,
    Array(ArrayState),
    Object(ObjectState),
    Number(NumberState, NumberData),
    String,
}

#[derive(Debug)]
pub struct Parser<T> where T: Tokenizer {
    input: T,
    stack: Vec<State>,
    started: bool,
}

enum Transition {
    PopStack,
    ReadValue,
    PopRedo,
    Nothing,
}

impl<T> Parser<T> where T: Tokenizer {
    pub fn new(tokenizer: T) -> Self {
        Parser {
            input: tokenizer,
            stack: vec![State::Root],
            started: false,
        }
    }

    fn open_type<S>(&mut self, sink: &mut S, token: Token) -> PResult<()> where S: ParserSink {
        match token {

            // Quote signals start of a string.
            Token::Quote => {
                self.stack.push(State::String);
                sink.start_string();
                Ok(())
            },

            // Sign signals start of a number.
            Token::Sign(sign) => {
                let mut nd = NumberData::default();
                nd.sign = sign;
                self.stack.push(State::Number(NumberState::Integer, nd));
                Ok(())
            },
            Token::Number(num) => {
                let mut nd = NumberData::default();
                nd.integer = Some(num);
                self.stack.push(State::Number(NumberState::DotExponentEnd, nd));
                Ok(())
            },

            // Literals
            Token::Boolean(boolean) => { sink.push_bool(boolean); Ok(()) },
            Token::Null => { sink.push_null(); Ok(()) },

            // Composite objects.
            Token::ObjectOpen => {
                self.stack.push(State::Object(ObjectState::Start));
                sink.push_map();
                Ok(())
            },
            Token::ArrayOpen => {
                self.stack.push(State::Array(ArrayState::Start));
                sink.push_array();
                Ok(())
            },

            Token::ObjectClose => panic!("unexpected }"),
            Token::ArrayClose => panic!("Unexpected ]"),
            Token::Eof => panic!("Unexpected EOF"),
            _ => unreachable!(),
        }
    }

    pub fn parse<S>(&mut self, sink: &mut S) -> PResult<()> where S: ParserSink {
        loop {
            let single_state = self.stack.len() == 1;
            let started = self.started;

            let token = match self.input.token() {
                Ok(token) if !started || !single_state => token,
                Ok(token) => return Err(ParseError::ExpectedEof),
                Err(ParseError::Eof) if started && single_state => return Ok(()),
                Err(ParseError::Bail) => return Err(ParseError::Bail),
                Err(err) => return Err(err),
            };

            self.started = true;

            self.step(sink, token);
        }
    }

    fn step<S>(&mut self, sink: &mut S, token: Token) -> PResult<()>
        where S: ParserSink {
        println!("{:?}", token);

        // Matches on current state, and decides on a state transition.
        let transition = match self.stack.last_mut().unwrap() {

            &mut State::Root => Transition::ReadValue,

            &mut State::Array(ref mut arr_state @ ArrayState::Start) => {
                *arr_state = ArrayState::CommaEnd;
                Transition::ReadValue
            }

            &mut State::Array(ref mut arr_state @ ArrayState::CommaEnd) => {
                sink.pop_into_array();
                match token {
                    Token::Comma => {
                        *arr_state = ArrayState::Start;
                        Transition::Nothing
                    },
                    Token::ArrayClose => {
                        sink.finalize_array();
                        Transition::PopStack
                    },
                    _ => panic!("unexpected"),
                }
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Start) => {
                if token == Token::ObjectClose {
                    Transition::PopStack
                } else {
                    *obj_state = ObjectState::Key;
                    Transition::ReadValue
                }
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Key) => {
                if token != Token::Colon {
                    return Err(ParseError::UnexpectedToken {
                        pos: self.input.position(),
                        message: "expected :",
                    });
                }
                *obj_state = ObjectState::Colon;
                Transition::Nothing
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Colon) => {
                *obj_state = ObjectState::CommaEnd;
                Transition::ReadValue
            }

            &mut State::Object(ref mut arr_state @ ObjectState::CommaEnd) => {
                sink.pop_into_map();

                match token {
                    Token::Comma => {
                        *arr_state = ObjectState::Start;
                        Transition::Nothing
                    },
                    Token::ObjectClose => {
                        sink.finalize_map();
                        Transition::PopStack
                    },
                    _ => panic!("unexpected"),
                }
            }

            &mut State::String => {
                match token {
                    Token::Quote => {
                        sink.finalize_string();
                        Transition::PopStack
                    },
                    Token::StringSource(range) => {
                        sink.append_string_range(range);
                        Transition::Nothing
                    },
                    Token::StringSingle(character) => {
                        sink.append_string_single(character);
                        Transition::Nothing
                    },
                    token => panic!("{:?}", token),
                }
            }

            &mut State::Number(ref mut num_state, ref mut data) => {
                match num_state {
                    &mut NumberState::Integer => {
                        match token {
                            Token::Number(num) => {
                                *num_state = NumberState::DotExponentEnd;
                                data.integer = Some(num);
                                Transition::Nothing
                            },
                            _ => panic!("unexpected")
                        }
                    }
                    &mut NumberState::DotExponentEnd => {
                        match token {
                            Token::Dot => {
                                *num_state = NumberState::Decimal;
                                Transition::Nothing
                            },
                            Token::Exponent => {
                                *num_state = NumberState::ExponentSign;
                                Transition::Nothing
                            }
                            _ => {
                                sink.push_number(data.clone());
                                Transition::PopRedo
                            }
                        }
                    },
                    &mut NumberState::Decimal => {
                        match token {
                            Token::Number(num) => {
                                *num_state = NumberState::ExponentStartEnd;
                                data.decimal = Some(num);
                                Transition::Nothing
                            },
                            _ => panic!("unexpected"),
                        }
                    }
                    &mut NumberState::ExponentStartEnd => {
                        match token {
                            Token::Exponent => {
                                *num_state = NumberState::ExponentSign;
                                Transition::Nothing
                            },
                            _ => {
                                sink.push_number(data.clone());
                                Transition::PopRedo
                            }
                        }
                    }
                    &mut NumberState::ExponentSign => {
                        match token {
                            Token::Sign(sign) => {
                                *num_state = NumberState::Exponent;
                                data.exponent_sign = sign;
                                Transition::Nothing
                            },
                            Token::Number(num) => {
                                data.exponent = Some(num);
                                sink.push_number(data.clone());
                                Transition::PopStack
                            },
                            _ => panic!("unexpected"),
                        }
                    }
                    &mut NumberState::Exponent => {
                        match token {
                            Token::Number(num) => {
                                data.exponent = Some(num);
                                sink.push_number(data.clone());
                                Transition::PopStack
                            },
                            _ => panic!("unexpected"),
                        }
                    }
                }
            }
        };

        // Matches on state transition, makes change on stack.
        match transition {
            Transition::ReadValue => {
                self.open_type(sink, token)
            },
            Transition::PopStack => {
                self.stack.pop().unwrap();
                Ok(())
            },
            Transition::PopRedo => {
                self.stack.pop().unwrap();
                self.step(sink, token)
            },
            Transition::Nothing => Ok(()),
        }
    }

}
