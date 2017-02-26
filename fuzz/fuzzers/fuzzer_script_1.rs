#![no_main]
extern crate libfuzzer_sys;
extern crate iterative_json_parser;

use iterative_json_parser::source::string::{VecSource, VecSourceB};
use iterative_json_parser::{Parser, Source, Sink};
use iterative_json_parser::ParseError;
use iterative_json_parser::sink::into_enum::{EnumSink, Json};
use iterative_json_parser::input::{SourceSink, BailVariant};

fn parse_to_enum_inner<Src>(mut ss: SourceSink<Src, EnumSink>, print: bool) -> Result<Json, ParseError<BailVariant<Src::Bail, ()>>> where Src: Source {
    let mut parser = Parser::new();
    loop {
        match parser.run(&mut ss) {
            Ok(()) => {
                if print {
                    println!("Internal state: {:?}", parser);
                    println!("Sink stack: {:?}", ss.sink.stack);
                }
                return Ok(ss.sink.to_result());
            },
            Err(ParseError::SourceBail(_)) => continue,
            Err(err) => {
                if print {
                    println!("Internal state: {:?}", parser);
                    println!("Sink stack: {:?}", ss.sink.stack);
                }
                return Err(err);
            },
        }
    };
}

fn parse_to_enum(data_bytes: &[u8]) -> Result<Json, ParseError<BailVariant<(), ()>>> {
    let mut bailing_ss = SourceSink {
        source: VecSourceB::new(data_bytes.to_vec()),
        sink: EnumSink::new(data_bytes),
    };
    let bailing_result = parse_to_enum_inner(bailing_ss, false);

    let mut ss = SourceSink {
        source: VecSource::new(data_bytes.to_vec()),
        sink: EnumSink::new(data_bytes),
    };
    let result = parse_to_enum_inner(ss, false);

    assert_eq!(bailing_result, result);
    return result;
}

#[export_name="rust_fuzzer_test_input"]
pub extern fn go(data: &[u8]) {
    let _ = parse_to_enum(data);
}
