//!
//! Bincode-based Dencoder impl
//!

use serde::{Serialize, de::DeserializeOwned};

use super::Dencoder;

///
/// Dencoder implemented over Bincode
///
#[derive(Debug)]
#[deprecated(
    since = "0.2.0",
    note = "bincode is unmaintained, use the bitcode Dencoder if possible"
)]
pub struct BincodeDencoder;

#[allow(deprecated)]
impl Dencoder for BincodeDencoder {
    fn encode<T: Serialize>(value: T) -> Result<Vec<u8>, super::Error> {
        bincode::serialize(&value).map_err(|e| super::Error::Encode(e.to_string()))
    }

    fn decode<U: DeserializeOwned>(value: Vec<u8>) -> Result<U, super::Error> {
        bincode::deserialize(&value).map_err(|e| super::Error::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]

    use serde::{Deserialize, Serialize};

    use super::BincodeDencoder;
    use crate::actors::remote::dencoder::Dencoder;

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

        let foo_enc = BincodeDencoder::encode(foo.clone()).unwrap();

        let foo_dec = BincodeDencoder::decode(foo_enc).unwrap();

        assert_eq!(foo, foo_dec);
    }
}
