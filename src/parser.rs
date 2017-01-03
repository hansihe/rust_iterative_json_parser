use ::PResult;
use ::tokenizer::{ Token, Tokenizer };
use ::sink::ParserSink;

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
enum State {
    Root,
    Array(ArrayState),
    Object(ObjectState),
}

#[derive(Debug)]
pub struct Parser {
    input: Box<Tokenizer>,
    stack: Vec<State>,
}

enum Transition {
    PopStack,
    ReadValue,
    Nothing,
}

impl Parser {
    pub fn new(tokenizer: Box<Tokenizer>) -> Self {
        Parser {
            input: tokenizer,
            stack: vec![State::Root],
        }
    }

    fn open_type<S>(&mut self, sink: &mut S, token: Token) -> PResult<()> where S: ParserSink {
        match token {
            Token::String(string) => { sink.push_string(&string); Ok(()) },
            Token::Number(num) => { sink.push_number(num); Ok(()) },
            Token::Boolean(boolean) => { sink.push_bool(boolean); Ok(()) },
            Token::Null => { sink.push_null(); Ok(()) },

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

            Token::ObjectClose => Err("unexpected }"),
            Token::ArrayClose => Err("Unexpected ]"),
            Token::Eof => Err("Unexpected EOF"),
            _ => unreachable!(),
        }
    }

    pub fn step<S>(&mut self, sink: &mut S) -> PResult<bool> where S: ParserSink {
        let token = self.input.token();

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
                *obj_state = ObjectState::Key;
                Transition::ReadValue
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Key) => {
                if token != Token::Colon {
                    return Err("expected :");
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
        };

        // Matches on state transition, makes change on stack.
        match transition {
            Transition::ReadValue => self.open_type(sink, token)?,
            Transition::PopStack => { self.stack.pop().unwrap(); },
            Transition::Nothing => (),
        }

        let finished = self.stack.len() == 1;
        if finished && self.input.token() != Token::Eof {
                return Err("expected EOF");
        }
        Ok(finished)
    }
}
