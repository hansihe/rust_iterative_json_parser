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
            Ok(()) => {
                println!("Internal state: {:?}", parser);
                println!("Sink stack: {:?}", ss.sink.stack);
                return Ok(ss.sink.to_result());
            },
            Err(ParseError::SourceBail(_)) => continue,
            Err(err) => {
                println!("Internal state: {:?}", parser);
                println!("Sink stack: {:?}", ss.sink.stack);
                return Err(err);
            },
        }
    }
}

macro_rules! o {
    { $( $key:expr => $value:expr ),* } => {
        Json::Object(vec![
            $( ($key.to_owned(), $value) ),*
        ])
    };
}
macro_rules! a {
    [ $( $value:expr ),* ] => {
        Json::Array(vec![
            $( $value ),*
        ])
    };
}
macro_rules! v {
    (true) => { Json::Boolean(true) };
    (false) => { Json::Boolean(false) };
    (null) => { Json::Null };
}
macro_rules! s {
    ($string:expr) => { Json::String($string.to_owned()) };
}
macro_rules! n {
    ($string:expr) => { Json::Number($string.to_owned()) };
}

/// Read in an empty object.
#[test]
fn empty_object() {
    let input = r#"{}"#;
    let result = parse_to_enum(input.as_bytes());
    let expected = o!{};
    assert_eq!(result, Ok(expected));
}

/// Read in a very basic object.
#[test]
fn simple_object() {
    let input = "{\"foo\": true}\n";
    let result = parse_to_enum(input.as_bytes());
    let expected = o!{"foo" => v!(true)};
    assert_eq!(result, Ok(expected));
}

/// Numbers can be represented in several forms in JSON. Make sure they all
/// work as intended.
#[test]
fn numbers() {
    let input = r#"[0, 0.0, 12.5, 1e12, -1, -92.34e-85]"#;
    let result = parse_to_enum(input.as_bytes());
    let expected = a![
        n!("+0.0e+1"),
        n!("+0.0e+1"),
        n!("+12.5e+1"),
        n!("+1.0e+12"),
        n!("-1.0e+1"),
        n!("-92.34e-85")
    ];
    assert_eq!(result, Ok(expected));
}

///// Bail should work at any point in the input sequence.
///// Bails are represented by the ampersand character, make sure it can
///// occur at all points in the input sequence.
//#[test]
//fn simple_bails() {
//    let input = r#"&{&"f&oo"&:& &tr&ue&,& &"bar": &-&1&2&.&3&e&-&5&}&"#;
//    let result = parse_to_enum(input.as_bytes());
//    let expected = o!{
//        "f&oo" => v!(true),
//        "bar" => n!("-1&2&.3&e-5&")
//    };
//    assert_eq!(result, Ok(expected));
//}

/// Test a more complete example with many types.
#[test]
fn full_parse() {
    let input = r#"{"foo": [null, true, false], "woo": [], "bar": -1.23e-7, "baz": "woo\""}"#;

    let result = parse_to_enum(input.as_bytes());

    let expected = o!{
        "foo" => a![v!(null), v!(true), v!(false)],
        "woo" => a![],
        "bar" => n!("-1.23e-7"),
        "baz" => s!("woo\"")
    };

    assert_eq!(result, Ok(expected));
}

/// When we finish reading in a single value, we should expect EOF.
/// Make sure a new top level value can't be started once we finish
/// reading one.
#[test]
fn expect_eof() {
    let input = r#"{"woo": false}{"#;
    let result = parse_to_enum(input.as_bytes());
    assert!(result.is_err());
}

#[test]
fn root_string_value() {
    {
        let input = "\"test\"";
        let result = parse_to_enum(input.as_bytes());
        assert_eq!(result, Ok(s!("test")));
    }
}

#[test]
fn root_number_value() {
    {
        let input = "12";
        let result = parse_to_enum(input.as_bytes());
        assert_eq!(result, Ok(n!("+12.0e+1")));
    }
    {
        let input = "12e12";
        let result = parse_to_enum(input.as_bytes());
        assert_eq!(result, Ok(n!("+12.0e+12")));
    }
    {
        let input = "12e12";
        let result = parse_to_enum(input.as_bytes());
        assert_eq!(result, Ok(n!("+12.0e+12")));
    }
}

#[test]
fn root_literals() {
    {
        let input = "true";
        let result = parse_to_enum(input.as_bytes());
        assert_eq!(result, Ok(v!(true)));
    }
}

#[test]
fn short_encodings_utf8() {
    {
        let input = [b'"', 0b1111_0000, 0b1000_0000, 0b1000_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_err());
    }
    {
        let input = [b'"', 0b1110_0000, 0b1000_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_err());
    }
    {
        let input = [b'"', 0b1110_0000, 0b1001_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_err());
    }
    {
        let input = [b'"', 0b1111_0000, 0b1010_0000, 0b1000_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_ok());
    }
    {
        let input = [b'"', 0b1111_0000, 0b1001_0000, 0b1000_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_ok());
    }
    {
        let input = [b'"', 0b1110_0000, 0b1010_0000, 0b1000_0000, b'"'];
        let result = parse_to_enum(&input);
        assert!(result.is_ok());
    }
}

#[test]
fn fuzz_panic_1() {
    let input = b"{\"4\": }";
    let result = parse_to_enum(input);
    assert!(result.is_err());
}

#[test]
fn fuzz_panic_2() {
    let input = b"{{";
    let result = parse_to_enum(input);
    assert!(result.is_err());
}

#[test]
fn fuzz_panic_3() {
    let input = b"{\"1\":{\"1\": \"foo\", \"2\": 12, \"3\": 12.222, \"4\\udE12.222, \"4\\ueE\x00\x00\x00l12\"\"1";
    let result = parse_to_enum(input);
    assert!(result.is_err());

}

#[test]
fn fuzz_panic_4() {
    let input = b"\"\xc2\"";
    let result = parse_to_enum(input);
    assert!(result.is_err());
}
