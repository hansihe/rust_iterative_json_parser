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
mod utf8;

pub use error::{ParseError, Unexpected};

pub use input::{Range, Pos};
pub use source::{Source, PeekResult};
pub use sink::Sink;


pub use parser::NumberData;
pub use tokenizer::{TokenizerState};
pub use TokenizerState as Parser;

pub use input::{Bailable, SourceSink, BailVariant};

pub type PResult<T, SourceBail> = Result<T, ParseError<SourceBail>>;
