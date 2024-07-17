//!
//! local and remote implementation of the actor model.
//!
//! remote actors are capability-based. traits are exposed for creating new net and message encoding/decoding layers.
//!

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

pub mod actors;
pub mod messaging;

#[cfg(test)]
#[allow(unused)]
mod tests {
    use tokio::sync::OnceCell;

    static TRACING: OnceCell<()> = OnceCell::const_new();

    pub(crate) async fn init_tracing() {
        TRACING
            .get_or_init(|| async {
                tracing_subscriber::fmt::init();
            })
            .await;
    }
}
