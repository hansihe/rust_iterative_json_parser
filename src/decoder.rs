use ::parser::ParserState;
use ::tokenizer::TokenizerState;

struct Decoder {
    parser: ParserState,
    tokenizer: TokenizerState,
}

impl Decoder {
    fn new() -> Self {
        Decoder {
            parser: ParserState::new(),
            tokenizer: TokenizerState::new(),
        }
    }

}
