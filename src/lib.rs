#![cfg_attr(feature = "use_simd", feature(platform_intrinsics, cfg_target_feature, asm))]
#[cfg(all(feature = "use_simd", target_feature = "sse2", target_feature = "ssse3"))]
extern crate simd;

pub mod parser;
pub mod tokenizer;
pub mod sink;
pub mod input;
pub mod error;
pub mod source;
pub mod decoder;

pub type PResult<T, SourceBail, SinkBail> = Result<T, error::ParseError<SourceBail, SinkBail>>;

pub use parser::{ParserState, NumberData};
pub use source::{Source, SourceError};
pub use sink::Sink;
pub use error::ParseError;
pub use input::{Range, Pos};
pub use tokenizer::{TokenizerState, SS};

pub use TokenizerState as Parser;
