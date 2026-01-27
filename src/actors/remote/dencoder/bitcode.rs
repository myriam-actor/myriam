//!
//! Bitcode-based Dencoder implementation
//!

use crate::actors::remote::dencoder::Dencoder;

///
/// Bitcode-based Dencoder implementation
///
#[derive(Debug)]
pub struct BitcodeDencoder;

impl Dencoder for BitcodeDencoder {
    fn encode<T: serde::Serialize>(value: T) -> Result<Vec<u8>, super::Error> {
        bincode::serialize(&value).map_err(|e| super::Error::Encode(e.to_string()))
    }

    fn decode<U: serde::de::DeserializeOwned>(value: Vec<u8>) -> Result<U, super::Error> {
        bincode::deserialize(&value).map_err(|e| super::Error::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::actors::remote::dencoder::{Dencoder, bitcode::BitcodeDencoder};

    const TEST_STRING: &str = "a ü string ⅞123";

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Foo {
        a: u32,
        b: String,
        c: Vec<i32>,
    }

    impl Foo {
        fn new() -> Self {
            Self {
                a: 123,
                b: TEST_STRING.into(),
                c: vec![1, 2, 3],
            }
        }
    }

    #[test]
    fn decode_and_encode() {
        let foo = Foo::new();

        let foo_enc = BitcodeDencoder::encode(foo.clone()).unwrap();

        let foo_dec = BitcodeDencoder::decode(foo_enc).unwrap();

        assert_eq!(foo, foo_dec);
    }
}
