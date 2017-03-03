use ::Bailable;
use ::input::Range;
pub use ::parser::NumberData;

pub mod debug_print;
pub mod into_enum;

pub trait Sink: Bailable {
    fn push_map(&mut self, pos: Position);
    fn push_array(&mut self, pos: Position);

    fn push_number(&mut self, pos: Position, integer: NumberData) -> Result<(), Self::Bail>;
    fn push_bool(&mut self, pos: Position, boolean: bool) -> Result<(), Self::Bail>;
    fn push_null(&mut self, pos: Position) -> Result<(), Self::Bail>;

    fn start_string(&mut self, pos: StringPosition);
    fn append_string_range(&mut self, string: Range);
    fn append_string_single(&mut self, character: u8);
    fn append_string_codepoint(&mut self, codepoint: char);
    fn finalize_string(&mut self, pos: StringPosition) -> Result<(), Self::Bail>;

    fn finalize_array(&mut self, pos: Position) -> Result<(), Self::Bail>;
    fn finalize_map(&mut self, pos: Position) -> Result<(), Self::Bail>;

    fn pop_into_map(&mut self);
    fn pop_into_array(&mut self);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Position {
    Root,
    MapValue,
    ArrayValue,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StringPosition {
    Root,
    MapKey,
    MapValue,
    ArrayValue,
}

impl StringPosition {
    pub fn to_position(self) -> Position {
        match self {
            StringPosition::Root => Position::Root,
            StringPosition::MapKey => panic!(),
            StringPosition::MapValue => Position::MapValue,
            StringPosition::ArrayValue => Position::ArrayValue,
        }
    }
}
