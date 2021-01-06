use anyhow::Result;
use log::info;
use twitchchat::{connector, AsyncRunner, RunnerError, UserConfig};

pub async fn connect(user_config: &UserConfig, channel: &str) -> Result<AsyncRunner, RunnerError> {
    // create a connector using ``smol``, this connects to Twitch.
    // you can provide a different address with `custom`
    // this can fail if DNS resolution cannot happen
    let connector = connector::SmolConnectorTls::twitch()?;

    info!("Connecting to twitch chat server");
    // create a new runner. this is a provided async 'main loop'
    // this method will block until you're ready
    let mut runner = AsyncRunner::connect(connector, user_config).await?;

    info!("Connected as: {}", runner.identity.username());

    runner.join(channel).await?;
    info!("Joined '{}'!", channel);

    Ok(runner)
}
