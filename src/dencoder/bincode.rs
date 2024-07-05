use serde::{de::DeserializeOwned, Serialize};

use super::Dencoder;

#[derive(Debug)]
pub struct BincodeDencoder;

impl Dencoder for BincodeDencoder {
    fn encode<T: Serialize>(value: T) -> Result<Vec<u8>, super::Error> {
        bincode::serialize(&value).map_err(|_| super::Error::Encode)
    }

    fn decode<U: DeserializeOwned>(value: Vec<u8>) -> Result<U, super::Error> {
        bincode::deserialize(&value).map_err(|_| super::Error::Decode)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::BincodeDencoder;
    use crate::dencoder::Dencoder;

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
