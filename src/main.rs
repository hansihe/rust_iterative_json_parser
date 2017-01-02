use std::fmt::Debug;

type PResult<T> = Result<T, &'static str>;

trait ParserSink {
    fn push_map(&mut self);
    fn push_array(&mut self);
    fn push_string(&mut self, string: &str);
    fn push_number(&mut self, integer: f64);
    fn push_bool(&mut self, boolean: bool);
    fn push_null(&mut self);
    fn pop_into_map(&mut self, key: &str);
    fn pop_into_array(&mut self);
}

struct PrintSink {}

impl PrintSink {
    fn new() -> PrintSink {
        PrintSink {}
    }
}

impl ParserSink for PrintSink {
    fn push_map(&mut self) { println!("push_map"); }
    fn push_array(&mut self) { println!("push_array"); }
    fn push_string(&mut self, string: &str) { println!("push_string {:?}", string); }
    fn push_number(&mut self, num: f64) { println!("push_float {:?}", num); }
    fn push_bool(&mut self, val: bool) { println!("push_bool {:?}", val); }
    fn push_null(&mut self) { println!("push_none"); }
    fn pop_into_map(&mut self, key: &str) { println!("pop_into_map with key {:?}", key); }
    fn pop_into_array(&mut self) { println!("pop_into_array"); }
}

#[derive(Debug, PartialEq)]
enum Token {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    ObjectOpen,
    ObjectClose,
    ArrayOpen,
    ArrayClose,
    Comma,
    Colon,
    Eof,
}

trait Tokenizer: Debug {
    fn token(&mut self) -> Token;
}

#[derive(Debug)]
struct TestTokenStream {
    tokens: Vec<Token>,
}

impl TestTokenStream {
    fn new(tokens: Vec<Token>) -> Self {
        TestTokenStream {
            tokens: tokens,
        }
    }
}

impl Tokenizer for TestTokenStream {
    fn token(&mut self) -> Token {
        self.tokens.pop().unwrap()
    }
}

#[derive(Debug)]
enum Json {
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
    Array(ArrayState, Vec<Json>),
    Object(ObjectState, Vec<(String, Json)>, Option<String>),
}

#[derive(Debug)]
struct Parser {
    input: Box<Tokenizer>,
    stack: Vec<State>,
    ret: Option<Json>,
}

enum Transition {
    PopStack,
    ReadValue,
    Nothing,
}

impl Parser {
    fn new(tokenizer: Box<Tokenizer>) -> Self {
        Parser {
            input: tokenizer,
            stack: vec![State::Root],
            ret: None,
        }
    }

    fn string(&mut self) -> String {
        "".to_string()
    }

    fn open_type<S>(&mut self, sink: &mut S, token: Token) -> PResult<()> where S: ParserSink {
        match token {
            Token::String(string) => { self.ret = Some(Json::String(string)); Ok(()) },
            Token::Number(num) => { self.ret = Some(Json::Number(num)); Ok(()) },
            Token::Boolean(boolean) => { self.ret = Some(Json::Boolean(boolean)); Ok(()) },
            Token::Null => { self.ret = Some(Json::Null); Ok(()) },
            Token::ObjectOpen => { self.stack.push(State::Object(ObjectState::Start, Vec::new(), None)); Ok(()) },
            Token::ArrayOpen => { self.stack.push(State::Array(ArrayState::Start, Vec::new())); Ok(()) },

            //Token::String(string) => { self.ret = Some(Json::String(string)); Ok(()) },
            //Token::Number(num) => { self.ret = Some(Json::Number(num)); Ok(()) },
            //Token::Boolean(boolean) => { self.ret = Some(Json::Boolean(boolean)); Ok(()) },
            //Token::Null => { self.ret = Some(Json::Null); Ok(()) },
            //Token::ObjectOpen => { self.stack.push(State::Object(ObjectState::Start, Vec::new(), None)); Ok(()) },
            //Token::ArrayOpen => { self.stack.push(State::Array(ArrayState::Start, Vec::new())); Ok(()) },

            Token::ObjectClose => Err("unexpected }"),
            Token::ArrayClose => Err("Unexpected ]"),
            Token::Eof => Err("Unexpected EOF"),
            _ => unreachable!(),
        }
    }

    fn step<S>(&mut self, sink: &mut S) -> PResult<bool> where S: ParserSink {
        println!("State: {:?}", self);
        let token = self.input.token();
        println!("Token: {:?}", token);
        println!("=================================");

        // borrowchk trickery.
        let mut ret = None;
        std::mem::swap(&mut ret, &mut self.ret);

        // Matches on current state, and decides on a state transition.
        let transition = match self.stack.last_mut().unwrap() {

            &mut State::Root => Transition::ReadValue,

            &mut State::Array(ref mut arr_state @ ArrayState::Start, _) => {
                *arr_state = ArrayState::CommaEnd;
                Transition::ReadValue
            }

            &mut State::Array(ref mut arr_state @ ArrayState::CommaEnd, ref mut arr) => {
                arr.push(ret.unwrap());
                match token {
                    Token::Comma => {
                        *arr_state = ArrayState::Start;
                        Transition::Nothing
                    },
                    Token::ArrayClose => Transition::PopStack,
                    _ => panic!("unexpected"),
                }
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Start, _, _) => {
                *obj_state = ObjectState::Key;
                Transition::ReadValue
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Key, _, ref mut key @ None) => {
                if token != Token::Colon {
                    return Err("expected :");
                }
                *obj_state = ObjectState::Colon;
                *key = Some(ret.unwrap().extract_string());
                Transition::Nothing
            }

            &mut State::Object(ref mut obj_state @ ObjectState::Colon, _, _) => {
                *obj_state = ObjectState::CommaEnd;
                Transition::ReadValue
            }

            &mut State::Object(ref mut arr_state @ ObjectState::CommaEnd, ref mut obj, ref mut key) => {
                // borrowchk trickery.
                let mut key_i: Option<String> = None;
                std::mem::swap(key, &mut key_i);

                obj.push((key_i.unwrap(), ret.unwrap()));

                match token {
                    Token::Comma => {
                        *arr_state = ObjectState::Start;
                        Transition::Nothing
                    },
                    Token::ObjectClose => Transition::PopStack,
                    _ => panic!("unexpected"),
                }
            }

            _ => unreachable!(),
        };

        // Matches on state transition, makes change on stack.
        match transition {
            Transition::ReadValue => self.open_type(sink, token)?,
            Transition::PopStack => match self.stack.pop().unwrap() {
                State::Array(_, val) => {
                    self.ret = Some(Json::Array(val));
                },
                State::Object(_, val, _) => {
                    self.ret = Some(Json::Object(val));
                }
                _ => unreachable!(),
            },
            Transition::Nothing => (),
        }

        let finished = self.stack.len() == 1 && self.ret.is_some();
        if finished && self.input.token() != Token::Eof {
                return Err("expected EOF");
        }
        Ok(finished)
    }
}

fn test_parse() -> PResult<()> {
    let tokenizer = TestTokenStream::new(
        vec![
            Token::Eof,
            Token::ObjectClose,
            Token::ArrayClose,
            Token::String("bar".to_string()),
            Token::Comma,
            Token::Number(1.5f64),
            Token::ArrayOpen,
            Token::Colon,
            Token::String("foo".to_string()),
            Token::ObjectOpen,
        ]);

    let mut parser = Parser::new(Box::new(tokenizer));

    let mut sink = PrintSink::new();

    loop {
        match parser.step(&mut sink) {
            Ok(true) => {
                println!("Finish!\n{:?}", parser.ret);
                return Ok(())
            },
            Ok(false) => continue,
            Err(msg) => {
                println!("Error: \n{:?}\n{:?}", msg, parser);
                return Err(msg);
            }
        }
    }
}

fn main() {
    test_parse();
}
