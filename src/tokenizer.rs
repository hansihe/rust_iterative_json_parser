use std::fmt::Debug;

use ::PResult;
use ::error::ParseError;
use ::input::{Pos, Range};
use ::source::{Source, SourceError};
use ::sink::Sink;

static UTF8_CHAR_WIDTH: [u8; 256] = [
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x1F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x3F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x5F
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1, // 0x7F
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 0x9F
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, // 0xBF
    0,0,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
    2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2, // 0xDF
    3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3, // 0xEF
    4,4,4,4,4,0,0,0,0,0,0,0,0,0,0,0, // 0xFF
];

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Token {
    // Number parts
    Sign(bool),
    Number(Range),
    Dot,
    Exponent,

    // String parts
    Quote,
    StringSource(Range),
    StringSingle(u8),
    StringCodepoint(u32),

    // Rest
    Boolean(bool),
    Null,
    ObjectOpen,
    ObjectClose,
    ArrayOpen,
    ArrayClose,
    Comma,
    Colon,

    // Special
    Eof,
}

pub struct TokenSpan {
    pub token: Token,
    pub span: Range,
}

#[derive(Debug, Copy, Clone)]
enum StringState {
    None,
    Codepoint(u8),
    StartEscape,
    UnicodeEscape(u8, u32),
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
pub struct TokenizerState {
    state: TokenState,
}

impl TokenizerState {

    pub fn new() -> TokenizerState {
        TokenizerState {
            state: TokenState::None,
        }
    }

    fn skip_whitespace<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        while match source.peek_char() {
            Ok(' ') => true,
            Ok('\t') => true,
            Ok('\n') => true,
            Ok('\r') => true,
            Ok(_) => false,
            Err(SourceError::Bail(bail)) => return Err(ParseError::SourceBail(bail)),
            Err(SourceError::Eof) => return Err(ParseError::Eof),
        } { source.skip(1); }
        Ok(())
    }

    fn read_char<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<char, <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        match source.peek_char() {
            Ok(character) => {
                source.skip(1);
                Ok(character)
            },
            Err(SourceError::Bail(bail)) => return Err(ParseError::SourceBail(bail)),
            Err(SourceError::Eof) => return Err(ParseError::Eof),
        }
    }

    /// Called when we want to expect a literal.
    ///
    /// Will return `final_token` when the literal is successfully read.
    fn lit<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk, data: &'static [u8], final_token: Token) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        self.state = TokenState::Lit(data, 0, final_token);
        self.token(source, sink)
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
    fn do_lit<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        let token = match self.state {
            TokenState::Lit(ref mut data, ref mut curr_pos, ref token) => {

                // Go forwards from the position where we left off
                // until the end of the literal string.
                for pos in *curr_pos..data.len() {
                    let pos_char = data[pos] as char;

                    match source.peek_char() {

                        // We matched a single character exactly.
                        // Keep going.
                        Ok(character) if character == pos_char =>
                            source.skip(1),

                        // We got some unexpected character.
                        // Return a parse error.
                        Ok(character) =>
                            return Err(ParseError::Unexpected(source.position())),

                        // We reached EOF.
                        // This should not happen in the middle of a literal,
                        // return a parse error.
                        Err(SourceError::Eof) =>
                            return Err(ParseError::Unexpected(source.position())),

                        // We got a bail signal.
                        // Store our state so that we can pick up where
                        // we left off.
                        Err(SourceError::Bail(bt)) => {
                            *curr_pos = pos;
                            return Err(ParseError::SourceBail(bt));
                        }
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
    fn do_str<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        let token = match self.state {
            TokenState::String(ref mut start, ref mut string_state) => {
                loop {
                    match (*string_state, source.peek_char()) {

                        // We reached the end of the string (unescaped quote).
                        // Return the last part of the string now, quote token
                        // next time.
                        (StringState::None, Ok('"')) => {
                            let range = Range::new(*start, source.position());
                            *start = source.position();

                            *string_state = StringState::End;
                            source.skip(1);

                            return Ok(Token::StringSource(range));
                        },
                        (StringState::End, _) => {
                            break;
                        },

                        // Got a backslash, emit the string part we have and
                        // expect something escaped next.
                        (StringState::None, Ok('\\')) => {
                            let range = Range::new(*start, source.position());

                            *string_state = StringState::StartEscape;
                            source.skip(1);

                            return Ok(Token::StringSource(range));
                        },

                        // Normal characters.
                        // Skip and emit a range when we reach something else.
                        (StringState::None, Ok(_)) => {
                            source.skip(1);
                        },

                        // The last character was a backslash.
                        // We should expect an escaped character.
                        (StringState::StartEscape, Ok('"')) => {
                            *start = source.position();
                            *string_state = StringState::None;
                            source.skip(1);
                        },
                        (StringState::StartEscape, Ok('\\')) => {
                            *start = source.position();
                            *string_state = StringState::None;
                            source.skip(1);
                        },
                        (StringState::StartEscape, Ok('/')) => {
                            *start = source.position();
                            *string_state = StringState::None;
                            source.skip(1);
                        },
                        (StringState::StartEscape, Ok('b')) => {
                            *string_state = StringState::None;
                            source.skip(1);
                            *start = source.position();
                            return Ok(Token::StringSingle(0x62));
                        },
                        (StringState::StartEscape, Ok('f')) => {
                            *string_state = StringState::None;
                            source.skip(1);
                            *start = source.position();
                            return Ok(Token::StringSingle(0x66));
                        },
                        (StringState::StartEscape, Ok('n')) => {
                            *string_state = StringState::None;
                            source.skip(1);
                            *start = source.position();
                            return Ok(Token::StringSingle('\n' as u8));
                        },
                        (StringState::StartEscape, Ok('r')) => {
                            *string_state = StringState::None;
                            source.skip(1);
                            *start = source.position();
                            return Ok(Token::StringSingle('\r' as u8));
                        },
                        (StringState::StartEscape, Ok('t')) => {
                            *string_state = StringState::None;
                            source.skip(1);
                            *start = source.position();
                            return Ok(Token::StringSingle('\t' as u8));
                        },
                        (StringState::StartEscape, Ok('u')) => {
                            *string_state = StringState::UnicodeEscape(4, 0);
                            source.skip(1);
                        },
                        (StringState::StartEscape, Ok(_)) => unimplemented!(),

                        (StringState::UnicodeEscape(ref mut count, ref mut codepoint),
                         Ok(character)) => {
                            *codepoint <<= 4;
                            *count -= 1;

                            let byte = character as u8;
                            match character {
                                'A'...'F' => *codepoint |= (byte - b'A' + 10) as u32,
                                'a'...'f' => *codepoint |= (byte - b'a' + 10) as u32,
                                '0'...'9' => *codepoint |= (byte - b'0') as u32,
                                _ => return Err(ParseError::Unexpected(source.position())),
                            }

                            source.skip(1);
                            if *count == 0 {
                                *string_state = StringState::None;
                                *start = source.position();
                                return Ok(Token::StringCodepoint(*codepoint));
                            }
                            *string_state = StringState::UnicodeEscape(*count, *codepoint);
                        }

                        // Errors
                        (_, Err(SourceError::Eof)) =>
                            return Err(ParseError::Unexpected(source.position())),
                        (_, Err(SourceError::Bail(bt))) =>
                            return Err(ParseError::SourceBail(bt)),
                    }
                }
            },
            _ => unreachable!(),
        };
        self.state = TokenState::None;
        Ok(Token::Quote)
    }

    fn do_num<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<Token, <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        match self.state {
            TokenState::Number(start) => {
                loop {
                    match source.peek_char() {

                        // Walk through numbers
                        Ok('0'...'9') => source.skip(1),

                        // ... any other character breaks
                        Ok(_) => break,

                        // Errors
                        Err(SourceError::Eof) =>
                            return Err(ParseError::Unexpected(source.position())),
                        Err(SourceError::Bail(bt)) =>
                            return Err(ParseError::SourceBail(bt)),
                    }
                }

                self.state = TokenState::None;
                Ok(Token::Number(Range::new(start, source.position())))
            }
            _ => unreachable!(),
        }
    }

    pub fn token<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        match self.state {
            TokenState::Lit(_, _, _) => self.do_lit(source, sink),
            TokenState::String(_, _) => self.do_str(source, sink),
            TokenState::Number(_) => self.do_num(source, sink),
            TokenState::None => {
                self.skip_whitespace(source, sink)?;

                match self.read_char(source, sink)? {
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
                    't' => self.lit(source, sink, b"rue", Token::Boolean(true)),
                    'f' => self.lit(source, sink, b"alse", Token::Boolean(false)),
                    'n' => self.lit(source, sink, b"ull", Token::Null),
                    '0'...'9' => {
                        let start = source.position().0 - 1;
                        self.state = TokenState::Number(start.into());
                        self.do_num(source, sink)
                    },
                    '"' => {
                        self.state = TokenState::String(source.position(),
                                                        StringState::None);
                        Ok(Token::Quote)

                    },
                    _ => Err(ParseError::Unexpected(source.position())),
                }
            }
        }
    }

    //pub fn run<Src, Snk>(&mut self, source: &mut Src, sink: &mut Snk) -> PResult<(), Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
    //    loop {
    //        match self.state {
    //            TokenState::Lit(_, _, _) => self.do_lit(source, sink),
    //            TokenState::String(_, _) => self.do_str(source, sink),
    //            TokenState::Number(_) => self.do_num(source, sink),
    //            TokenState::None => {
    //                self.skip_whitespace(source, sink)?;

    //                match self.read_char()
    //            }
    //        }
    //    }
    //}

}
