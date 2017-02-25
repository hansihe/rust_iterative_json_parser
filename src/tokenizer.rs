use ::PResult;
use ::error::{ParseError, Unexpected};
use ::input::{Pos, Range};
use ::source::{Source, SourceError};
use ::sink::Sink;
use ::parser::ParserState;
use ::utf8;

#[derive(Debug, Clone)]
pub struct SS<Src, Snk> where Src: Source, Snk: Sink {
    pub source: Src,
    pub sink: Snk,
}

#[derive(Debug, Copy, Clone)]
enum StringState {
    None(utf8::DecodeState),
    StartEscape,
    UnicodeEscape(u8, u32),
    End,
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
}


impl TokenizerState {

    pub fn new() -> TokenizerState {
        TokenizerState {
            state: TokenState::None,
            parser: ParserState::new(),

            string_state: StringState::None(utf8::UTF8_ACCEPT),
            string_start: 0.into(),
        }
    }

    fn skip_whitespace<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
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

    fn read_char<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<u8, <Src as Source>::Bail> where Src: Source, Snk: Sink {
        match ss.source.peek_char() {
            Ok(character) => {
                ss.source.skip(1);
                Ok(character)
            },
            Err(SourceError::Bail(bail)) => return Err(ParseError::SourceBail(bail)),
            Err(SourceError::Eof) => return Err(ParseError::Eof),
        }
    }

    #[cfg(not(feature = "use_simd"))]
    fn validate_utf8<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, init_state: utf8::DecodeState, initial_character: u8) -> PResult<utf8::DecodeState, <Src as Source>::Bail> where Src: Source, Snk: Sink {
        let mut curr_char = initial_character;
        let mut state = init_state;

        loop {
            state = utf8::decode(state, curr_char);
            if utf8::should_stop(state) {
                match state {
                    utf8::UTF8_REJECT =>
                        return Err(ParseError::Unexpected(ss.source.position(), Unexpected::InvalidUtf8)),
                    utf8::UTF8_SPECIAL =>
                        break,
                    _ => unreachable!(),
                }
            }

            ss.source.skip(1);

            curr_char = match ss.source.peek_char() {
                Ok(character) => character,
                Err(SourceError::Eof) =>
                    return Err(ParseError::Eof),
                Err(SourceError::Bail(bail)) => {
                    // When we receive a bail signal, we need to set
                    // the string state so that we can continue from
                    // where we left off.
                    self.string_state = StringState::None(state);
                    return Err(ParseError::SourceBail(bail));
                },
            }
        }

        self.string_state = StringState::None(state);
        Ok(state)
    }

    #[cfg(all(feature = "use_simd", target_feature = "sse2", target_feature = "ssse3"))]
    fn validate_utf8<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, initial_character: u8) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
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

            let mut length = UTF8_CHAR_WIDTH[curr as usize];
            if length == 0 {
                if curr == b'\\' || curr == b'"' {
                    break 'fallback;
                }
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
    fn do_str<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail> where Src: Source, Snk: Sink {

        loop {
            match (self.string_state, ss.source.peek_char()) {

                (StringState::None(utf8::UTF8_REJECT), _) => {
                    return Err(ParseError::Unexpected(ss.source.position(), Unexpected::InvalidUtf8));
                },

                // Processes characters normally.
                // This should be the fast-path as it is the most common.
                (StringState::None(state), Ok(character)) => {
                    match (character, state) {
                        // We reached the end of the string (unescaped quote).
                        // Return the last part of the string now, quote token
                        // next time.
                        (b'"', utf8::UTF8_ACCEPT) | (b'"', utf8::UTF8_SPECIAL) => {
                            let range = Range::new(self.string_start, ss.source.position());
                            self.string_start = ss.source.position();
                            self.string_state = StringState::End;
                            ss.source.skip(1);

                            if !(range.start == range.end) {
                                self.parser.token_string_range(ss, range)?;
                            }
                        },
                        // Got a backslash, emit the string part we have and
                        // expect something escaped next.
                        (b'\\', utf8::UTF8_ACCEPT) | (b'\\', utf8::UTF8_SPECIAL) => {
                            let range = Range::new(self.string_start, ss.source.position());

                            self.string_state = StringState::StartEscape;
                            ss.source.skip(1);

                            if !(range.start == range.end) {
                                self.parser.token_string_range(ss, range)?;
                            }
                        },
                        // Normal characters.
                        // Use fast-path.
                        (_, utf8::UTF8_SPECIAL) => unreachable!(),
                        (_, utf8_state) =>
                            self.string_state = StringState::None(
                                self.validate_utf8(ss, utf8_state, character)?),
                    }
                },

                (StringState::End, _) => {
                    break;
                },

                // The last character was a backslash.
                // We should expect an escaped character.
                (StringState::StartEscape, Ok(character)) => {
                    match character {
                        b'"' | b'\\' | b'/' => {
                            self.string_start = ss.source.position();
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
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
                                _ => return Err(ParseError::Unexpected(ss.source.position(), Unexpected::InvalidEscape)),
                            };
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
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
                        _ => return Err(ParseError::Unexpected(ss.source.position(), Unexpected::InvalidEscapeHex)),
                    }

                    ss.source.skip(1);
                    if *count == 0 {
                        self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                        self.string_start = ss.source.position();
                        if let Some(character) = ::std::char::from_u32(*codepoint) {
                            self.parser.token_string_codepoint(ss, character)?;
                        } else {
                            return Err(ParseError::Unexpected(ss.source.position(), Unexpected::InvalidUtf8))
                        }
                    } else {
                        self.string_state = StringState::UnicodeEscape(*count, *codepoint);
                    }
                },

                // Errors
                (_, Err(SourceError::Eof)) =>
                    return Err(ParseError::Unexpected(ss.source.position(), Unexpected::Eof)),
                (_, Err(SourceError::Bail(bt))) =>
                    return Err(ParseError::SourceBail(bt)),
            }
        }

        self.state = TokenState::None;
        self.parser.token_quote(ss)
    }

    fn do_num<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>, start: Pos) -> PResult<(), <Src as Source>::Bail> where Src: Source, Snk: Sink {
        loop {
            match ss.source.peek_char() {

                // Walk through numbers
                Ok(b'0'...b'9') => ss.source.skip(1),

                // ... any other character breaks
                Ok(_) => break,

                // Errors
                Err(SourceError::Eof) => break,
                Err(SourceError::Bail(bt)) =>
                    return Err(ParseError::SourceBail(bt)),
            }
        }

        self.state = TokenState::None;
        let pos = ss.source.position();
        self.parser.token_number(ss, Range::new(start, pos))
    }

    fn do_run<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail> where Src: Source, Snk: Sink {
        loop {
            match self.state {
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
                            self.string_state = StringState::None(utf8::UTF8_ACCEPT);
                            self.state = TokenState::String;
                            self.parser.token_quote(ss)?;
                        }
                        _ => return Err(ParseError::Unexpected(ss.source.position(), Unexpected::Character)),
                    }
                }
            }
        }
    }

    pub fn run<Src, Snk>(&mut self, ss: &mut SS<Src, Snk>) -> PResult<(), Src::Bail> where Src: Source, Snk: Sink {
        match self.do_run(ss) {
            Ok(()) => unreachable!(),
            Err(ParseError::Eof) => {
                self.parser.finish(ss);
                if self.parser.finished() {
                    Ok(())
                } else {
                    Err(ParseError::Unexpected(ss.source.position(), Unexpected::Eof))
                }
            },
            Err(err) => Err(err),
        }
    }

}
