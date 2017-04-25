use ::PResult;
use ::Bailable;
use ::error::{ParseError, Unexpected};
use ::input::{Pos, Range};
use ::source::{Source, PeekResult};
use ::sink::Sink;
use ::parser::ParserState;
use ::utf8;

#[derive(Debug, Copy, Clone)]
enum StringState {
    None(utf8::DecodeState),
    StartEscape,
    UnicodeEscape(u8, u32, Option<u32>),
    StartUnicodeContinuation(StartContinuationState, u32),
    End,
}

#[derive(Debug, Copy, Clone)]
enum StartContinuationState {
    Slash,
    Uchar,
}

#[derive(Debug, Copy, Clone)]
enum TokenState {
    None,
    String,
    Number(Pos),
}

#[derive(Debug)]
pub struct TokenizerState {
    state: TokenState,
    parser: ParserState,

    string_state: StringState,
    string_start: Pos,

    to_end: bool,
}

macro_rules! unexpected {
    ($ss:expr, $reason:expr) => {
        Err(ParseError::Unexpected($ss.position(), $reason))
    }
}

impl TokenizerState {
    pub fn new() -> TokenizerState {
        TokenizerState {
            state: TokenState::None,
            parser: ParserState::new(),

            string_state: StringState::None(utf8::UTF8_ACCEPT),
            string_start: 0.into(),

            to_end: true,
        }
    }

    pub fn new_read_rest() -> TokenizerState {
        let mut parser = TokenizerState::new();
        parser.to_end = false;
        parser
    }

    fn skip_whitespace<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail>
        where SS: Source + Sink + Bailable
    {
        while match ss.peek_char() {
            PeekResult::Ok(b' ') => true,
            PeekResult::Ok(b'\t') => true,
            PeekResult::Ok(b'\n') => true,
            PeekResult::Ok(b'\r') => true,
            PeekResult::Ok(_) => false,
            PeekResult::Bail(bail) => return Err(ParseError::SourceBail(bail)),
            PeekResult::Eof => return Err(ParseError::Eof),
        } {
            ss.skip(1);
        }
        Ok(())
    }

    fn read_char<SS>(&mut self, ss: &mut SS) -> PResult<u8, SS::Bail>
        where SS: Source + Sink + Bailable
    {
        match ss.peek_char() {
            PeekResult::Ok(character) => {
                ss.skip(1);
                Ok(character)
            }
            PeekResult::Bail(bail) => return Err(ParseError::SourceBail(bail)),
            PeekResult::Eof => return Err(ParseError::Eof),
        }
    }

    fn validate_utf8<SS>(&mut self,
                         ss: &mut SS,
                         init_state: utf8::DecodeState,
                         initial_character: u8)
                         -> PResult<utf8::DecodeState, SS::Bail>
        where SS: Source + Sink + Bailable
    {
        let mut curr_char = initial_character;
        let mut state = init_state;

        loop {
            state = utf8::decode(state, curr_char);

            match state {
                utf8::UTF8_REJECT => {
                    return unexpected!(ss, Unexpected::InvalidUtf8);
                }
                utf8::UTF8_SPECIAL => {
                    self.string_state = StringState::None(state);
                    return Ok(state);
                }
                _ => (),
            }

            ss.skip(1);
            curr_char = match ss.peek_char() {
                PeekResult::Ok(character) => character,
                PeekResult::Eof => return Err(ParseError::Eof),
                PeekResult::Bail(bail) => {
                    // When we receive a bail signal, we need to set
                    // the string state so that we can continue from
                    // where we left off.
                    self.string_state = StringState::None(state);
                    return Err(ParseError::SourceBail(bail));
                }
            };
        }

    }

    // Continues processing on a string value in the JSON.
    fn do_str<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail>
        where SS: Source + Sink + Bailable
    {

        loop {
            match (self.string_state, ss.peek_char()) {

                (StringState::None(utf8::UTF8_REJECT), _) => {
                    return unexpected!(ss, Unexpected::InvalidUtf8);
                }

                // Processes characters normally.
                // This should be the fast-path as it is the most common.
                (StringState::None(state), PeekResult::Ok(character)) => {
                    match (character, state) {
                        // We reached the end of the string (unescaped quote).
                        // Return the last part of the string now, quote token
                        // next time.
                        (b'"', utf8::UTF8_ACCEPT) |
                        (b'"', utf8::UTF8_SPECIAL) => {
                            let range = Range::new(self.string_start, ss.position());
                            self.string_start = ss.position();
                            self.string_state = StringState::End;
                            ss.skip(1);

                            if !(range.start == range.end) {
                                self.parser.token_string_range(ss, range)?;
                            }
                        }
                        // Got a backslash, emit the string part we have and
                        // expect something escaped next.
                        (b'\\', utf8::UTF8_ACCEPT) |
                        (b'\\', utf8::UTF8_SPECIAL) => {
                            let range = Range::new(self.string_start, ss.position());

                            self.string_state = StringState::StartEscape;
                            ss.skip(1);

                            if !(range.start == range.end) {
                                self.parser.token_string_range(ss, range)?;
                            }
                        }
                        // Normal characters.
                        // Use fast-path.
                        (_, utf8::UTF8_SPECIAL) => unreachable!(),
                        (_, utf8_state) => {
                            self.string_state =
                                StringState::None(self.validate_utf8(ss, utf8_state, character)?)
                        }
                    }
                }

                (StringState::End, _) => {
                    self.state = TokenState::None;
                    return self.parser.token_quote(ss);
                }

                // The last character was a backslash.
                // We should expect an escaped character.
                (StringState::StartEscape, PeekResult::Ok(character)) => {
                    match character {
                        b'"' | b'\\' | b'/' => {
                            self.string_start = ss.position();
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                            ss.skip(1);
                        }
                        b'u' => {
                            self.string_state = StringState::UnicodeEscape(4, 0, None);
                            ss.skip(1);
                        }
                        _ => {
                            let escaped = match character {
                                b'b' => 0x08,
                                b'f' => 0x0c,
                                b'n' => b'\n',
                                b'r' => b'\r',
                                b't' => b'\t',
                                _ => return unexpected!(ss, Unexpected::InvalidEscape),
                            };
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                            ss.skip(1);
                            self.string_start = ss.position();
                            self.parser.token_string_single(ss, escaped)?;
                        }
                    }
                }

                // We hit the end of a unicode escape sequence that was not preceeded
                // by a UTF-16 surrogate. Check if the codepoint is a surrogate, and
                // keep going.
                (StringState::UnicodeEscape(0, codepoint, None), PeekResult::Ok(_)) => {
                    if codepoint >= 0xd800 && codepoint <= 0xdbff {
                        self.string_state =
                            StringState::StartUnicodeContinuation(StartContinuationState::Slash,
                                                                  (codepoint - 0xd800) << 10);
                    } else {
                        self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                        self.string_start = ss.position();
                        if let Some(character) = ::std::char::from_u32(codepoint) {
                            self.parser.token_string_codepoint(ss, character)?;
                        } else {
                            return unexpected!(ss, Unexpected::InvalidUtf8);
                        }
                    }
                }

                // We hit the end of a unicode escape sequence that WAS preceeded by a
                // UTF-16 surrogate. Join them and validate.
                (StringState::UnicodeEscape(0, lower, Some(upper)), PeekResult::Ok(_)) => {
                    if lower >= 0xdc00 && lower <= 0xdfff {
                        self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                        self.string_start = ss.position();

                        let num = (upper | (lower - 0xdc00)) + 0x10000;
                        if let Some(character) = ::std::char::from_u32(num) {
                            self.parser.token_string_codepoint(ss, character)?;
                        } else {
                            return unexpected!(ss, Unexpected::InvalidUtf8);
                        }
                    } else {
                        return unexpected!(ss, Unexpected::InvalidUtf8);
                    }
                }

                // We hit a unicode escape sigil, and need to 4ead the next n
                // bytes (as hex) into a character.
                (StringState::UnicodeEscape(ref mut count, ref mut codepoint, lower),
                 PeekResult::Ok(character)) => {
                    *codepoint <<= 4;
                    *count -= 1;

                    let byte = character as u8;
                    match character {
                        b'A'...b'F' => *codepoint |= (byte - b'A' + 10) as u32,
                        b'a'...b'f' => *codepoint |= (byte - b'a' + 10) as u32,
                        b'0'...b'9' => *codepoint |= (byte - b'0') as u32,
                        _ => return unexpected!(ss, Unexpected::InvalidEscapeHex),
                    }

                    ss.skip(1);
                    self.string_state = StringState::UnicodeEscape(*count, *codepoint, lower);
                }

                (StringState::StartUnicodeContinuation(StartContinuationState::Slash, lower),
                 PeekResult::Ok(character)) => {
                    match character {
                        b'\\' => {
                            self.string_state = StringState::StartUnicodeContinuation(
                                StartContinuationState::Uchar, lower);
                            ss.skip(1);
                        }
                        _ => return unexpected!(ss, Unexpected::InvalidEscape),
                    }
                }
                (StringState::StartUnicodeContinuation(StartContinuationState::Uchar, lower),
                 PeekResult::Ok(character)) => {
                    match character {
                        b'u' => {
                            self.string_state = StringState::UnicodeEscape(4, 0, Some(lower));
                            ss.skip(1);
                        }
                        _ => return unexpected!(ss, Unexpected::InvalidEscape),
                    }
                }


                // Errors
                (_, PeekResult::Eof) => return unexpected!(ss, Unexpected::Eof),
                (_, PeekResult::Bail(bt)) => return Err(ParseError::SourceBail(bt)),
            }
        }
    }

    fn do_num<SS>(&mut self, ss: &mut SS, start: Pos) -> PResult<(), SS::Bail>
        where SS: Source + Sink + Bailable
    {
        loop {
            match ss.peek_char() {

                // Walk through numbers
                PeekResult::Ok(b'0'...b'9') => ss.skip(1),

                // ... any other character breaks
                PeekResult::Ok(_) => break,

                // Errors
                PeekResult::Eof => break,
                PeekResult::Bail(bt) => return Err(ParseError::SourceBail(bt)),
            }
        }

        self.state = TokenState::None;
        let pos = ss.position();
        self.parser.token_number(ss, Range::new(start, pos))
    }

    fn do_run<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail>
        where SS: Source + Sink + Bailable
    {
        loop {
            match self.state {
                TokenState::String => self.do_str(ss)?,
                TokenState::Number(start) => self.do_num(ss, start)?,
                TokenState::None => {
                    self.skip_whitespace(ss)?;

                    match self.read_char(ss)? {
                        b'{' => self.parser.token_object_open(ss)?,
                        b'}' => self.parser.token_object_close(ss)?,
                        b'[' => self.parser.token_array_open(ss)?,
                        b']' => self.parser.token_array_close(ss)?,
                        b',' => self.parser.token_comma(ss)?,
                        b':' => self.parser.token_colon(ss)?,
                        b'e' | b'E' => self.parser.token_exponent(ss)?,
                        b'.' => self.parser.token_dot(ss)?,
                        b'-' => self.parser.token_sign(ss, false)?,
                        b'+' => self.parser.token_sign(ss, true)?,
                        b't' => {
                            ss.skip(3);
                            self.parser.token_bool(ss, true)?;
                        }
                        b'f' => {
                            ss.skip(4);
                            self.parser.token_bool(ss, false)?;
                        }
                        b'n' => {
                            ss.skip(3);
                            self.parser.token_null(ss)?;
                        }
                        b'0'...b'9' => {
                            let start = ss.position().0 - 1;
                            self.state = TokenState::Number(start.into());
                            self.do_num(ss, start.into())?;
                        }
                        b'"' => {
                            self.string_start = ss.position();
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                            self.state = TokenState::String;
                            self.parser.token_quote(ss)?;
                        }
                        _ => return unexpected!(ss, Unexpected::Character),
                    }
                }
            }
        }
    }

    pub fn run<SS>(&mut self, ss: &mut SS) -> PResult<(), SS::Bail>
        where SS: Source + Sink + Bailable
    {
        self.parser.reentry(ss)?;

        if self.parser.finished() {
            Ok(())
        } else {
            match self.do_run(ss) {
                Ok(()) => unreachable!(),
                Err(ParseError::End) => {
                    self.parser.finish(ss)?;
                    Ok(())
                }
                Err(ParseError::Eof) => unexpected!(ss, Unexpected::Eof),
                err => err,
            }
        }

        // if !self.finished {
        //    match self.do_run(ss) {
        //        Ok(()) => unreachable!(),
        //        Err(ParseError::Eof) => {
        //            self.finished = true;
        //            self.parser.finish(ss)?;
        //        },
        //        Err(err) => return Err(err),
        //    }
        //

        // if self.finished {
        //    if self.parser.finished() {
        //        return Ok(());
        //    } else {
        //        return unexpected!(ss, Unexpected::Eof);
        //    }
        //

        // unreachable!();
    }
}
