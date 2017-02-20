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
        let mut parser = ParserState::new();
        println!("{:?}", parser.parse(&mut ss));
    }

}
