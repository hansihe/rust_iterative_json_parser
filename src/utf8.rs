// This module implements a modified version of the algorithm specified at
// http://bjoern.hoehrmann.de/utf-8/decoder/dfa/
// which also rejects control characters, double quotes and backslashes.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DecodeState(pub u8);

pub const UTF8_ACCEPT: DecodeState = DecodeState(0);
pub const UTF8_REJECT: DecodeState = DecodeState(14);
pub const UTF8_SPECIAL: DecodeState = DecodeState(254);

const CHAR_CLASSES: [u8; 256] = [
    12,12,12,12,12,12,12,12,12,12,12,12,12,12,12,12,
    12,12,12,12,12,12,12,12,12,12,12,12,12,12,12,12,
    0 ,0 ,13,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,
    0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,
    0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,
    0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,13,0 ,0 ,0 ,
    0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,
    0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,
    1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,1 ,
    9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,9 ,
    7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,
    7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,7 ,
    8 ,8 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,
    2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,2 ,
    10,3 ,3 ,3 ,3 ,3 ,3 ,3 ,3 ,3 ,3 ,3 ,3 ,4 ,3 ,3 ,
    11,6 ,6 ,6 ,5 ,8 ,8 ,8 ,8 ,8 ,8 ,8 ,8 ,8 ,8 ,8 ,
];

//const STATE_TRANSITIONS: [u8; 117] = [
//    0 ,13,26,39,65,104,91,13,13,13,52,78,13,
//    13,13,13,13,13,13 ,13,13,13,13,13,13,13,
//    13,0 ,13,13,13,13 ,13,0 ,13,0 ,13,13,13,
//    13,26,13,13,13,13 ,13,26,13,26,13,13,13,
//    13,13,13,13,13,13 ,13,26,13,13,13,13,13,
//    13,26,13,13,13,13 ,13,13,13,26,13,13,13,
//    13,13,13,13,13,13 ,13,39,13,39,13,13,13,
//    13,39,13,13,13,13 ,13,39,13,39,13,13,13,
//    13,39,13,13,13,13 ,13,13,13,13,13,13,13,
//];

// +---- character class ---->
// |
// | current state
// v

const STATE_TRANSITIONS: [u8; 126] = [
    0 ,14,28,42,70,112,98,14,14,14,56,84,14,254, // 0   + character class
    14,14,14,14,14,14 ,14,14,14,14,14,14,14,14,  // 14
    14,0 ,14,14,14,14 ,14,0 ,14,0 ,14,14,14,14,  // 28
    14,28,14,14,14,14 ,14,28,14,28,14,14,14,14,  // 42
    14,14,14,14,14,14 ,14,28,14,14,14,14,14,14,  // 56
    14,28,14,14,14,14 ,14,14,14,28,14,14,14,14,  // 70
    14,14,14,14,14,14 ,14,42,14,42,14,14,14,14,  // 84
    14,42,14,14,14,14 ,14,42,14,42,14,14,14,14,  // 98
    14,42,14,14,14,14 ,14,14,14,14,14,14,14,14,  // 112
];

//const STATE_TRANSITIONS: [u8; 117] = [
//    0,1,2,3,5,8,7,1,1,1,4,6,1,255,
//    1,1,1,1,1,1,1,1,1,1,1,1,1,255,
//    1,0,1,1,1,1,1,0,1,0,1,1,1,255,
//    1,2,1,1,1,1,1,2,1,2,1,1,1,255,
//    1,1,1,1,1,1,1,2,1,1,1,1,1,255,
//    1,2,1,1,1,1,1,1,1,2,1,1,1,255,
//    1,1,1,1,1,1,1,3,1,3,1,1,1,255,
//    1,3,1,1,1,1,1,3,1,3,1,1,1,255,
//    1,3,1,1,1,1,1,1,1,1,1,1,1,255,
//];

#[inline(always)]
pub fn decode(state: DecodeState, byte: u8) -> DecodeState {
    let typ = CHAR_CLASSES[byte as usize];

    // Baseline
    DecodeState(STATE_TRANSITIONS[(state.0 + typ) as usize])

    // 14% slower
    //unsafe {
    //    DecodeState(*STATE_TRANSITIONS.get_unchecked((state.0 + typ) as usize))
    //}
}

#[inline(always)]
pub fn should_stop(state: DecodeState) -> bool {
    (state.0 & 0b00001111) == 14
}

#[cfg(test)]
mod tests {
    use utf8::*;

    #[test]
    fn valid_ascii() {
        let mut state = UTF8_ACCEPT;
        for c in b"test" {
            state = decode(state, *c);
            assert!(state != UTF8_REJECT);
        }
    }

    #[test]
    fn fuzz_panic_1() {
        let mut state = UTF8_ACCEPT;
        for c in b"\xc2" {
            state = decode(state, *c);
            assert!(state != UTF8_REJECT);
        }
    }

}
