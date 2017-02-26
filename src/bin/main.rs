extern crate iterative_json_parser;

use std::fs::File;
use std::io::Read;

use iterative_json_parser::source::string::VecSource;
use iterative_json_parser::sink::into_enum::EnumSink;
use iterative_json_parser::TokenizerState;
use iterative_json_parser::input::{SourceSink};

fn main() {
    let mut data = Vec::<u8>::new();
    File::open("issue90.json").unwrap().read_to_end(&mut data).unwrap();

    for _ in 0..100 {
        let mut ss = SourceSink {
            source: VecSource::new(data.clone()),
            sink: EnumSink::new(&data),
        };
        let mut parser = TokenizerState::new();
        let ret = parser.run(&mut ss);
        println!("{:?} {:?}", ret, parser);
    }

}
