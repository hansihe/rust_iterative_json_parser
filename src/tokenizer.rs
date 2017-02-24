use ::PResult;
use ::error::ParseError;
use ::input::{Pos, Range};
use ::source::{Source, SourceError};
use ::sink::Sink;
use ::parser::ParserState;

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

#[derive(Debug, Clone)]
pub struct SS<Src, Snk> where Src: Source, Snk: Sink {
    pub source: Src,
    pub sink: Snk,
}

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
    StringCodepoint(char),

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
    String,
    Number(Pos),
    Lit(&'static [u8], usize, Token),
}

#[derive(Debug)]
pub struct TokenizerState {
    state: TokenState,
    parser: ParserState,

    string_state: StringState,
    string_start: Pos,
}


impl TokenizerState {

    pub fn new() -> TokenizerState {
        TokenizerState {
            state: TokenState::None,
            parser: ParserState::new(),

            string_state: StringState::None,
            string_start: 0.into(),
        }
    }

    fn skip_whitespace<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        while match ss.source.peek_char() {
            Ok(b' ') => true,
            Ok(b'\t') => true,
            Ok(b'\n') => true,
            Ok(b'\r') => true,
            Ok(_) => false,
            Err(SourceError::Bail(bail)) => return Err(ParseError::SourceBail(bail)),
            Err(SourceError::Eof) => return Err(ParseError::Eof),
        } { ss.source.skip(1); }
        Ok(())
    }

    fn read_char<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<u8, <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        match ss.source.peek_char() {
            Ok(character) => {
                ss.source.skip(1);
                Ok(character)
            },
            Err(SourceError::Bail(bail)) => return Err(ParseError::SourceBail(bail)),
            Err(SourceError::Eof) => return Err(ParseError::Eof),
        }
    }

    /// Called when we want to expect a literal.
    ///
    /// Will return `final_token` when the literal is successfully read.
    //fn lit<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, data: &'static [u8], final_token: Token) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
    //    self.state = TokenState::Lit(data, 0, final_token);
    //    self.token(ss)
    //}

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
    fn do_lit<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<Token, Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        let token = match self.state {
            TokenState::Lit(ref mut data, ref mut curr_pos, ref token) => {

                // Go forwards from the position where we left off
                // until the end of the literal string.
                for pos in *curr_pos..data.len() {
                    match ss.source.peek_char() {

                        // We matched a single character exactly.
                        // Keep going.
                        Ok(character) if character == data[pos] =>
                            ss.source.skip(1),

                        // We got some unexpected character.
                        // Return a parse error.
                        Ok(_) =>
                            return Err(ParseError::Unexpected(ss.source.position())),

                        // We reached EOF.
                        // This should not happen in the middle of a literal,
                        // return a parse error.
                        Err(SourceError::Eof) =>
                            return Err(ParseError::Unexpected(ss.source.position())),

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

    #[cfg(not(feature = "use_simd"))]
    fn validate_utf8<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, initial_character: u8) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        let mut length = 0;
        let mut curr_char = initial_character;

        // There is some code-repetition here, but having this fast-path
        // more than doubles the speed of reading string data.
        // I would say it is worth it.
        loop {
            // When the length is 0, it means we have reached the boundry
            // of a new unicode character, and should perform a new
            // length lookup.
            if length == 0 {
                length = UTF8_CHAR_WIDTH[curr_char as usize];
                // If the length is 0 from the LUT, it means the character
                // just read was invalid UTF8. Report a parse error.
                if length == 0 {
                    return Err(ParseError::Unexpected(ss.source.position()));
                }
                // If we see some other actionable character, bail
                // from the fast-path, and do a full match.
                if curr_char == b'\\' || curr_char == b'"' {
                    break;
                }
            } else {
                // When in the middle of a UTF8 character, we simply
                // need to validate that the two most significant bits
                // of the byte are 10.
                let valid = (curr_char & 0b11000000) == 0b10000000;
                if !valid {
                    return Err(ParseError::Unexpected(ss.source.position()));
                }
            }

            length -= 1;
            ss.source.skip(1);

            curr_char = match ss.source.peek_char() {
                Ok(character) => character,
                Err(SourceError::Eof) =>
                    return Err(ParseError::Eof),
                Err(SourceError::Bail(bail)) => {
                    // When we receive a bail signal, we need to set
                    // the string state so that we can continue from
                    // where we left off.
                    if length == 0 {
                        self.string_state = StringState::None;
                    } else {
                        self.string_state = StringState::Codepoint(length);
                    }
                    return Err(ParseError::SourceBail(bail));
                },
            }
        }

        Ok(())
    }

    #[cfg(all(feature = "use_simd", target_feature = "sse2", target_feature = "ssse3"))]
    fn validate_utf8<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, initial_character: u8) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        use ::simd::x86::ssse3::Ssse3I8x16;
        use ::simd::x86::sse2::Sse2I8x16;
        use ::simd::u8x16;
        use ::simd::i8x16;
        use ::simd::bool8ix16;

        //extern "platform-intrinsic" {
        //    //fn x86_mm_shuffle_epi8(a: ::simd::i8x16, b: ::simd::i8x16) -> ::simd::i8x16;
        //}

        fn bsf(num: u32) -> u32 {
            let result: u32;
            unsafe {
                asm!(
                    "bsf $0, $1":
                    "=r"(result):
                    "r"(num)
                );
            }
            result
        }

        // Constants for SIMD
        let quote_char = u8x16::splat(b'"');
        let escape_char = u8x16::splat(b'\\');
        let utf8_mask = u8x16::splat(0b10000000);
        let reverse_mask = i8x16::new(
            15, 14, 13, 12,
            11, 10, 09, 08,
            07, 06, 05, 04,
            03, 02, 01, 00,
        );

        'fallback: loop {

            'simd: loop {
                let skippable: u32;
                {
                    let slice = match ss.source.peek_slice(16) {
                        Some(slice) => slice,
                        None => break 'simd,
                    };
                    let curr = u8x16::load(slice, 0);

                    let quotes = curr.eq(quote_char);
                    let escapes = curr.eq(escape_char);
                    let unicode = bool8ix16::from_repr((curr & utf8_mask).to_i8());
                    let breaks = quotes | escapes | unicode;

                    let breaks_int = breaks.to_repr().shuffle_bytes(reverse_mask).move_mask();
                    let breaks_int_filled = breaks_int | 0b1_00000000_00000000;

                    skippable = bsf(breaks_int_filled);
                }
                ss.source.skip(skippable as usize);

                // We hit a UTF8/escape/quote character. Fallback.
                // If we didn't, keep going with the fast-path.
                if skippable < 16 {
                    break 'simd;
                }
            }

            let curr = match ss.source.peek_char() {
                Ok(character) => character,
                Err(SourceError::Bail(bail)) => {
                    self.string_state = StringState::None;
                    return Err(ParseError::SourceBail(bail));
                },
                Err(SourceError::Eof) => {
                    return Err(ParseError::Eof);
                },
            };

            if curr == b'\\' || curr == b'"' {
                break 'fallback;
            }

            let mut length = UTF8_CHAR_WIDTH[curr as usize];
            if length == 0 {
                return Err(ParseError::Unexpected(ss.source.position()));
            }

            ss.source.skip(1);

            'codepoint_scan: loop {
                if length == 1 {
                    break 'codepoint_scan;
                }

                let curr = match ss.source.peek_char() {
                    Ok(character) => character,
                    Err(SourceError::Bail(bail)) => {
                        self.string_state = StringState::Codepoint(length);
                        return Err(ParseError::SourceBail(bail));
                    },
                    Err(SourceError::Eof) => {
                        return Err(ParseError::Eof);
                    },
                };

                let valid = (curr & 0b11000000) == 0b10000000;
                if !valid {
                    return Err(ParseError::Unexpected(ss.source.position()));
                }

                length -= 1;
                ss.source.skip(1);
            }

        }

        Ok(())
    }

    // Continues processing on a string value in the JSON.
    fn do_str<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {

        loop {
            match (self.string_state, ss.source.peek_char()) {

                // Processes characters normally.
                // This should be the fast-path as it is the most common.
                (StringState::None, Ok(character)) => {
                    match character {
                        // We reached the end of the string (unescaped quote).
                        // Return the last part of the string now, quote token
                        // next time.
                        b'"' => {
                            let range = Range::new(self.string_start, ss.source.position());
                            self.string_start = ss.source.position();
                            self.string_state = StringState::End;
                            ss.source.skip(1);

                            if !range.empty() {
                                self.parser.token_string_range(ss, range)?;
                            }
                        },
                        // Got a backslash, emit the string part we have and
                        // expect something escaped next.
                        b'\\' => {
                            let range = Range::new(self.string_start, ss.source.position());

                            self.string_state = StringState::StartEscape;
                            ss.source.skip(1);

                            if !range.empty() {
                                self.parser.token_string_range(ss, range)?;
                            }
                        },
                        // Normal characters.
                        // Skip and emit a range when we reach something else.
                        _ => {
                            self.validate_utf8(ss, character)?

                            //let mut length = 0;
                            //let mut curr_char = character;

                            //// There is some code-repetition here, but having this fast-path
                            //// more than doubles the speed of reading string data.
                            //// I would say it is worth it.
                            //loop {
                            //    // When the length is 0, it means we have reached the boundry
                            //    // of a new unicode character, and should perform a new
                            //    // length lookup.
                            //    if length == 0 {
                            //        length = UTF8_CHAR_WIDTH[curr_char as usize];
                            //        // If the length is 0 from the LUT, it means the character
                            //        // just read was invalid UTF8. Report a parse error.
                            //        if length == 0 {
                            //            return Err(ParseError::Unexpected(ss.source.position()));
                            //        }
                            //        // If we see some other actionable character, bail
                            //        // from the fast-path, and do a full match.
                            //        if curr_char == b'\\' || curr_char == b'"' {
                            //            break;
                            //        }
                            //    } else {
                            //        // When in the middle of a UTF8 character, we simply
                            //        // need to validate that the two most significant bits
                            //        // of the byte are 10.
                            //        let valid = (curr_char & 0b11000000) == 0b10000000;
                            //        if !valid {
                            //            return Err(ParseError::Unexpected(ss.source.position()));
                            //        }
                            //    }

                            //    length -= 1;
                            //    ss.source.skip(1);

                            //    curr_char = match ss.source.peek_char() {
                            //        Ok(character) => character,
                            //        Err(SourceError::Eof) =>
                            //            return Err(ParseError::Eof),
                            //        Err(SourceError::Bail(bail)) => {
                            //            // When we receive a bail signal, we need to set
                            //            // the string state so that we can continue from
                            //            // where we left off.
                            //            if length != 0 {
                            //                self.string_state = StringState::None;
                            //            } else {
                            //                self.string_state = StringState::Codepoint(length);
                            //            }
                            //            return Err(ParseError::SourceBail(bail));
                            //        },
                            //    }
                            //}

                            //// TODO: OPT: Check characters inline
                            //match length {
                            //    0 => return Err(ParseError::Unexpected(ss.source.position())),
                            //    1 => (),
                            //    length => {
                            //        self.string_state = StringState::Codepoint(length - 2)
                            //    },
                            //}
                            //ss.source.skip(1);
                        },
                    }
                },

                (StringState::End, _) => {
                    break;
                },

                // We are in the middle of reading a unicode codepoint.
                // Validate the next characters.
                (StringState::Codepoint(num_left), Ok(character)) => {
                    let valid = (character & 0b11000000) == 0b10000000;
                    if valid {
                        self.string_state = match num_left {
                            1 => StringState::None,
                            2 => StringState::Codepoint(1),
                            3 => StringState::Codepoint(2),
                            _ => unreachable!(),
                        };
                        ss.source.skip(1);
                    } else {
                        return Err(ParseError::Unexpected(ss.source.position()));
                    }
                },

                // The last character was a backslash.
                // We should expect an escaped character.
                (StringState::StartEscape, Ok(character)) => {
                    match character {
                        b'"' | b'\\' | b'/' => {
                            self.string_start = ss.source.position();
                            self.string_state = StringState::None;
                            ss.source.skip(1);
                        },
                        b'u' => {
                            self.string_state = StringState::UnicodeEscape(4, 0);
                            ss.source.skip(1);
                        },
                        _ => {
                            let escaped = match character {
                                b'b' => 0x62,
                                b'f' => 0x66,
                                b'n' => b'\n',
                                b'r' => b'\r',
                                b't' => b'\t',
                                _ => return Err(ParseError::Unexpected(ss.source.position())),
                            };
                            self.string_state = StringState::None;
                            ss.source.skip(1);
                            self.string_start = ss.source.position();
                            self.parser.token_string_single(ss, escaped)?;
                        },
                    }
                },

                // We hit a unicode escape sigil, and need to 4ead the next n
                // bytes (as hex) into a character.
                (StringState::UnicodeEscape(ref mut count, ref mut codepoint),
                 Ok(character)) => {
                    *codepoint <<= 4;
                    *count -= 1;

                    let byte = character as u8;
                    match character {
                        b'A'...b'F' => *codepoint |= (byte - b'A' + 10) as u32,
                        b'a'...b'f' => *codepoint |= (byte - b'a' + 10) as u32,
                        b'0'...b'9' => *codepoint |= (byte - b'0') as u32,
                        _ => return Err(ParseError::Unexpected(ss.source.position())),
                    }

                    ss.source.skip(1);
                    if *count == 0 {
                        self.string_state = StringState::None;
                        self.string_start = ss.source.position();
                        self.parser.token_string_codepoint(ss, ::std::char::from_u32(*codepoint).unwrap())?;
                    } else {
                        self.string_state = StringState::UnicodeEscape(*count, *codepoint);
                    }
                },

                // Errors
                (_, Err(SourceError::Eof)) =>
                    return Err(ParseError::Unexpected(ss.source.position())),
                (_, Err(SourceError::Bail(bt))) =>
                    return Err(ParseError::SourceBail(bt)),
            }
        }

        self.state = TokenState::None;
        self.parser.token_quote(ss)
    }

    fn do_num<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, start: Pos) -> PResult<(), <Src as Source>::Bail, <Snk as Sink>::Bail> where Src: Source, Snk: Sink {
        loop {
            match ss.source.peek_char() {

                // Walk through numbers
                Ok(b'0'...b'9') => ss.source.skip(1),

                // ... any other character breaks
                Ok(_) => break,

                // Errors
                Err(SourceError::Eof) =>
                    return Err(ParseError::Unexpected(ss.source.position())),
                Err(SourceError::Bail(bt)) =>
                    return Err(ParseError::SourceBail(bt)),
            }
        }

        self.state = TokenState::None;
        let pos = ss.source.position();
        self.parser.token_number(ss, Range::new(start, pos))
    }

    fn do_run<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        loop {
            match self.state {
                TokenState::Lit(..) => {
                    self.do_lit(ss)?;
                },
                TokenState::String => {
                    self.do_str(ss)?;
                },
                TokenState::Number(start) => {
                    self.do_num(ss, start)?;
                },
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
                            ss.source.skip(3);
                            self.parser.token_bool(ss, true)?;
                        },
                        b'f' => {
                            ss.source.skip(4);
                            self.parser.token_bool(ss, false)?;
                        }
                        b'n' => {
                            ss.source.skip(3);
                            self.parser.token_null(ss)?;
                        },
                        b'0'...b'9' => {
                            let start = ss.source.position().0 - 1;
                            self.state = TokenState::Number(start.into());
                            self.do_num(ss, start.into())?;
                        }
                        b'"' => {
                            self.string_start = ss.source.position();
                            self.string_state = StringState::None;
                            self.state = TokenState::String;
                            self.parser.token_quote(ss)?;
                        }
                        _ => return Err(ParseError::Unexpected(ss.source.position())),
                    }
                }
            }
        }
    }

    pub fn run<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail, Snk::Bail> where Src: Source, Snk: Sink {
        match self.do_run(ss) {
            Ok(()) => unreachable!(),
            Err(ParseError::Eof) => {
                if self.parser.finished() {
                    Ok(())
                } else {
                    Err(ParseError::Eof)
                }
            },
            Err(err) => Err(err),
        }
    }

}
