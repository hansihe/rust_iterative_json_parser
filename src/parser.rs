use ::PResult;
use ::tokenizer::{ Token, TokenizerState };
use ::sink::Sink;
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

#[derive(Debug)]
enum State {
    Root,
    Array(ArrayState),
    Object(ObjectState),
    Number(NumberState, NumberData),
    String,
}

#[derive(Debug)]
pub struct ParserState {
    tokenizer: TokenizerState,
    stack: Vec<State>,
    started: bool,
}

enum Transition {
    PopStack,
    ReadValue,
    PopRedo,
    Nothing,
}

impl ParserState {

    pub fn new() -> Self {
        ParserState {
            tokenizer: TokenizerState::new(),
            stack: vec![State::Root],
            started: false,
        }
    }

    fn open_type<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk, token: Token) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
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
                nd.integer = num;
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

    pub fn parse<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        loop {
            let single_state = self.stack.len() == 1;
            let started = self.started;

            let token = match self.tokenizer.token(source, sink) {
                Ok(token) if !started || !single_state => token,
                Ok(token) => return Err(ParseError::UnexpectedToken(source.position(), token)),
                Err(ParseError::Eof) if started && single_state => return Ok(()),
                Err(err) => return Err(err),
            };

            self.started = true;

            self.step(source, sink, token)?;
        }
    }

    fn step<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk, token: Token) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {

        // Matches on current state, and decides on a state transition.
        let transition = match self.stack.last_mut().unwrap() {

            &mut State::Root => Transition::ReadValue,

            &mut State::Array(ref mut arr_state @ ArrayState::Start) => {
                if token == Token::ArrayClose {
                    Transition::PopStack
                } else {
                    *arr_state = ArrayState::CommaEnd;
                    Transition::ReadValue
                }
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
                    _ => return Err(ParseError::UnexpectedToken(source.position(), token)),
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
                    panic!("unexpected Colon");
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
                    Token::StringCodepoint(codepoint) => {
                        sink.append_string_codepoint(codepoint);
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
                                data.integer = num;
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
                self.open_type(source, sink, token)
            },
            Transition::PopStack => {
                self.stack.pop().unwrap();
                Ok(())
            },
            Transition::PopRedo => {
                self.stack.pop().unwrap();
                self.step(source, sink, token)
            },
            Transition::Nothing => Ok(()),
        }
    }

}
