use std::future::Future;

pub mod local;
pub mod remote;

pub trait Actor<I, O, E> {
    fn handler(&mut self, input: I) -> impl Future<Output = Result<O, E>> + Send;
}

#[cfg(test)]
mod tests {
    use super::Actor;

    pub(crate) struct Mult {
        pub a: u32,
    }

    #[derive(Debug, thiserror::Error)]
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
