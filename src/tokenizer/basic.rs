use std::fmt::Debug;

use ::PResult;
use super::{ Token, TokenSpan, Tokenizer };
use ::error::ParseError;
use ::input::{Pos, Range};
use ::source::Source;

#[derive(Debug, Copy, Clone)]
enum StringState {
    None,
    StartEscape,
    UnicodeEscape(usize),
    End,
}

#[derive(Debug, Copy, Clone)]
enum TokenState {
    None,
    String(Pos, StringState),
    Number(Pos),
    Lit(&'static [u8], usize, Token),
}

#[derive(Debug)]
pub struct BasicTokenizer<S> where S: Source + Debug {
    source: S,
    state: TokenState,
}

impl<S> BasicTokenizer<S> where S: Source + Debug {

    pub fn new(source: S) -> BasicTokenizer<S> {
        BasicTokenizer {
            source: source,
            state: TokenState::None,
        }
    }

    fn skip_whitespace(&mut self) -> PResult<()> {
        while match self.source.peek_char()? {
            ' ' => true,
            '\t' => true,
            '\n' => true,
            '\r' => true,
            _ => false,
        } { self.source.skip(1); }
        Ok(())
    }

    fn read_char(&mut self) -> PResult<char> {
        let val = self.source.peek_char()?;
        self.source.skip(1);
        Ok(val)
    }

    /// Called when we want to expect a literal.
    ///
    /// Will return `final_token` when the literal is successfully read.
    fn lit(&mut self, data: &'static [u8], final_token: Token) -> PResult<Token> {
        self.state = TokenState::Lit(data, 0, final_token);
        self.token()
    }

    /// Called when the current tokenizer state is TokenState::Lit,
    /// and does all processing related to that state.
    ///
    /// There are multiple ways this function can return:
    /// 1. We reached the end of the literal without any trouble,
    ///    reset the tokenizer state and return the Token specified
    ///    in the TokenState::Lit.
    /// 2. We hit some unexpected character/EOF. This is a normal
    ///    parse error.
    /// 3. We got the bail signal from the source. We store the current
    ///    position in the literal so that we can pick up in the next call.
    fn do_lit(&mut self) -> PResult<Token> {
        let token = match self.state {
            TokenState::Lit(ref mut data, ref mut curr_pos, ref token) => {

                // Go forwards from the position where we left off
                // until the end of the literal string.
                for pos in *curr_pos..data.len() {
                    let pos_char = data[pos] as char;

                    match self.source.peek_char() {

                        // We matched a single character exactly.
                        // Keep going.
                        Ok(character) if character == pos_char =>
                            self.source.skip(1),

                        // We got some unexpected character.
                        // Return a parse error.
                        Ok(character) =>
                            return Err(ParseError::UnexpectedChar {
                                pos: self.source.position(),
                                expected: Some(pos_char),
                            }),

                        // We reached EOF.
                        // This should not happen in the middle of a literal,
                        // return a parse error.
                        Err(ParseError::Eof) =>
                            return Err(ParseError::UnexpectedEof),

                        // We got a bail signal.
                        // Store our state so that we can pick up where
                        // we left off.
                        Err(ParseError::Bail) => {
                            *curr_pos = pos;
                            return Err(ParseError::Bail);
                        }

                        // peek_char should never return anything else.
                        // TODO: Represent this in the type system.
                        _ => unreachable!(),
                    }
                }
                *token
            },
            // Because a predicate to calling this is that the tokenizer
            // state is TokenizerState::Lit, all other branches are
            // unreachable.
            _ => unreachable!(),
        };

        self.state = TokenState::None;
        Ok(token)
    }

    // Continues processing on a string value in the JSON.
    fn do_str(&mut self) -> PResult<Token> {
        let token = match self.state {
            TokenState::String(ref mut start, ref mut string_state) => {
                loop {
                    match (*string_state, self.source.peek_char()) {

                        // We reached the end of the string (unescaped quote).
                        // Return the last part of the string now, quote token
                        // next time.
                        (StringState::None, Ok('"')) => {
                            let range = Range::new(*start, self.source.position());
                            *start = self.source.position();

                            *string_state = StringState::End;
                            self.source.skip(1);

                            return Ok(Token::StringSource(range));
                        },
                        (StringState::End, _) => {
                            break;
                        },

                        // Got a backslash, emit the string part we have and
                        // expect something escaped next.
                        (StringState::None, Ok('\\')) => {
                            let range = Range::new(*start, self.source.position());

                            *string_state = StringState::StartEscape;
                            self.source.skip(1);

                            return Ok(Token::StringSource(range));
                        },

                        // Normal characters.
                        // Skip and emit a range when we reach something else.
                        (StringState::None, Ok(_)) => {
                            self.source.skip(1);
                        },

                        // The last character was a backslash.
                        // We should expect an escaped character.
                        (StringState::StartEscape, Ok('"')) => {
                            *start = self.source.position();

                            *string_state = StringState::None;
                            self.source.skip(1);
                        },
                        (StringState::StartEscape, Ok(_)) => unimplemented!(),

                        // Errors
                        (_, Err(ParseError::Eof)) =>
                            return Err(ParseError::UnexpectedEof),
                        (_, Err(ParseError::Bail)) =>
                            return Err(ParseError::Bail),

                        _ => unreachable!(),
                    }
                }
            },
            _ => unreachable!(),
        };
        self.state = TokenState::None;
        Ok(Token::Quote)
    }

    fn do_num(&mut self) -> PResult<Token> {
        match self.state {
            TokenState::Number(start) => {
                loop {
                    match self.source.peek_char() {

                        // Walk through numbers
                        Ok('0'...'9') => self.source.skip(1),

                        // ... any other character breaks
                        Ok(_) => break,

                        // Errors
                        Err(ParseError::Eof) =>
                            return Err(ParseError::UnexpectedEof),
                        Err(ParseError::Bail) =>
                            return Err(ParseError::Bail),

                        _ => unreachable!(),
                    }
                }

                self.state = TokenState::None;
                Ok(Token::Number(Range::new(start, self.source.position())))
            }
            _ => unreachable!(),
        }
    }

}

impl<S> Tokenizer for BasicTokenizer<S> where S: Source + Debug {

    fn token(&mut self) -> PResult<Token> {
        match self.state {
            TokenState::Lit(_, _, _) => self.do_lit(),
            TokenState::String(_, _) => self.do_str(),
            TokenState::Number(_) => self.do_num(),
            TokenState::None => {
                self.skip_whitespace()?;

                match self.read_char()? {
                    '{' => Ok(Token::ObjectOpen),
                    '}' => Ok(Token::ObjectClose),
                    '[' => Ok(Token::ArrayOpen),
                    ']' => Ok(Token::ArrayClose),
                    ',' => Ok(Token::Comma),
                    ':' => Ok(Token::Colon),
                    'e' => Ok(Token::Exponent),
                    'E' => Ok(Token::Exponent),
                    '.' => Ok(Token::Dot),
                    '-' => Ok(Token::Sign(false)),
                    '+' => Ok(Token::Sign(true)),
                    't' => self.lit(b"rue", Token::Boolean(true)),
                    'f' => self.lit(b"alse", Token::Boolean(false)),
                    'n' => self.lit(b"ull", Token::Null),
                    '0'...'9' => {
                        let start = self.source.position().0 - 1;
                        self.state = TokenState::Number(start.into());
                        self.do_num()
                    },
                    '"' => {
                        self.state = TokenState::String(self.source.position(),
                                                        StringState::None);
                        Ok(Token::Quote)

                    },
                    _ => {
                        Err(ParseError::UnexpectedChar{
                            pos: self.source.position(),
                            expected: None,
                        })
                    },
                }
            }
        }
    }

    fn position(&self) -> Pos {
        self.source.position()
    }

}
