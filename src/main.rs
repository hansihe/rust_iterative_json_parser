#![cfg_attr(feature = "use_simd", feature(platform_intrinsics, cfg_target_feature, asm))]
#[cfg(all(feature = "use_simd", target_feature = "sse2", target_feature = "ssse3"))]
extern crate simd;


pub mod parser;
pub mod tokenizer;
pub mod sink;
pub mod input;
pub mod error;
pub mod source;

pub type PResult<T, SourceBail, SinkBail> = Result<T, error::ParseError<SourceBail, SinkBail>>;

pub use parser::{ ParserState, NumberData };
pub use source::Source;
pub use sink::Sink;
pub use error::ParseError;
pub use input::{ Range, Pos };

use std::fs::File;
use std::io::Read;

use source::string::VecSource;
use sink::into_enum::EnumSink;
use tokenizer::{TokenizerState, SS};

fn main() {
    let mut data = Vec::<u8>::new();
    File::open("issue90.json").unwrap().read_to_end(&mut data).unwrap();

    for i in 0..600 {
        let mut ss = SS {
            source: VecSource::new(data.clone()),
            sink: EnumSink::new(&data),
        };
        let mut parser = TokenizerState::new();
        let ret = parser.run(&mut ss);
        println!("{:?} {:?}", ret, parser);
    }

}
