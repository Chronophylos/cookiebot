use log::info;
use twitchchat::{connector, AsyncRunner, UserConfig};

pub async fn connect(user_config: &UserConfig, channel: &str) -> anyhow::Result<AsyncRunner> {
    // create a connector using ``smol``, this connects to Twitch.
    // you can provide a different address with `custom`
    // this can fail if DNS resolution cannot happen
    let connector = connector::SmolConnectorTls::twitch()?;

    info!("Connecting to twitch");
    // create a new runner. this is a provided async 'main loop'
    // this method will block until you're ready
    let mut runner = AsyncRunner::connect(connector, user_config).await?;

    info!("Connected with Identity: {:#?}", runner.identity);

    info!("Attempting to join '{}'", channel);
    runner.join(channel).await?;
    info!("Joined '{}'!", channel);

    Ok(runner)
}
