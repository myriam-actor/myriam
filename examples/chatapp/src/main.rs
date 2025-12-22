use color_eyre::eyre::{self, Result};
use messaging::{Messenger, MessengerCmd};
use models::{AppError, Report};
use myriam::{
    actors::remote::{
        self, dencoder::bitcode::BitcodeDencoder, netlayer::tor_layer::TorLayer, router::Router,
    },
    messaging::Message,
};
use tokio::sync::mpsc;
use tui::App;

mod messaging;
mod models;
mod tui;

#[tokio::main]
async fn main() -> Result<()> {
    install_hooks()?;

    let mut args = std::env::args().skip(1).take(2);

    let nickname = args
        .next()
        .ok_or(AppError::MissingArg("keystore nickname".to_string()))?;

    let port = args
        .next()
        .ok_or(AppError::MissingArg("service port".to_string()))?
        .parse()
        .map_err(|_| AppError::InvalidArg("not a valid port".to_string()))?;

    let router = Router::with_netlayer(TorLayer::new(nickname.clone(), port).await?, None).await?;

    let (tui_sender, tui_receiver) = mpsc::channel::<Report>(1024);

    let messenger = Messenger::new(nickname, tui_sender);
    let (messenger_local, mut messenger_untyped) =
        remote::spawn_untyped::<_, _, _, BitcodeDencoder>(messenger).await?;

    messenger_untyped.allow_mut(true);

    let addr = router.attach(messenger_untyped).await?;
    messenger_local
        .send(Message::TaskMut(MessengerCmd::Init(addr.clone())))
        .await?;

    let mut terminal = tui::init()?;

    let mut app = App::new(addr.clone(), messenger_local, tui_receiver);

    let tui_thread = tokio::task::spawn_blocking(move || {
        let _ = app.run(&mut terminal);
    });

    tui_thread.await?;

    tui::restore()?;
    Ok(())
}

pub fn install_hooks() -> Result<()> {
    let hook_builder = color_eyre::config::HookBuilder::default();
    let (panic_hook, eyre_hook) = hook_builder.into_hooks();

    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = tui::restore();
        panic_hook(panic_info);
    }));

    // convert from a color_eyre EyreHook to a eyre ErrorHook
    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(move |error| {
        let _ = tui::restore(); // ignore any errors as we are already failing
        eyre_hook(error)
    }))?;

    Ok(())
}
