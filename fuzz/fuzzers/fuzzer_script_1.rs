#![no_main]
extern crate libfuzzer_sys;
extern crate iterative_json_parser;

use iterative_json_parser::source::string::VecSource;
use iterative_json_parser::Parser;
use iterative_json_parser::ParseError;
use iterative_json_parser::SS;
use iterative_json_parser::sink::into_enum::{EnumSink, Json};

fn parse_to_enum(data_bytes: &[u8]) -> Result<Json, ParseError<()>> {
    let mut ss = SS {
        source: VecSource::new(data_bytes.to_vec()),
        sink: EnumSink::new(data_bytes),
    };

    let mut parser = Parser::new();

    loop {
        match parser.run(&mut ss) {
            Ok(()) => return Ok(ss.sink.to_result()),
            Err(ParseError::SourceBail(_)) => continue,
            Err(err) => {
                return Err(err);
            },
        }
    }
}


#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let _ = parse_to_enum(data);
}
