//!
//! root module for local and remote actors
//!

use std::future::Future;

pub mod local;

#[cfg(feature = "remote")]
pub mod remote;

///
/// main actor trait
///
/// type parameters `I`, `O` and `E` correspond to the handler's input, output and error, respectively.
///
pub trait Actor<I, O, E> {
    ///
    /// this actor's message handler
    ///
    fn handler(&mut self, input: I) -> impl Future<Output = Result<O, E>> + Send;
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "remote")]
    use serde::{Deserialize, Serialize};

    use super::Actor;

    pub(crate) struct Mult {
        pub a: u32,
    }

    #[derive(Debug, Clone, thiserror::Error)]
    #[cfg_attr(feature = "remote", derive(Serialize, Deserialize))]
    #[error("uh oh")]
    pub(crate) struct SomeError;

    impl Actor<u32, u32, SomeError> for Mult {
        async fn handler(&mut self, input: u32) -> Result<u32, SomeError> {
            Ok(input * self.a)
        }
    }

    #[tokio::test]
    async fn direct_message() {
        let mut a = Mult { a: 5 };

        assert_eq!(10, a.handler(2).await.unwrap());
    }
}
