use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

use rabbit_stuff::{
    impls::{MyMessage, OtherMessage, Pupil, SchoolAge},
    rabbit::{Rabbit, EXCHANGE, MESSAGE_TYPE, MESSAGE_TYPE_2},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    info!("hello!");

    let rabbit = Rabbit::new("amqp://localhost:5672").await?;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE,
            MyMessage {
                age: 25,
                name: "joseph".into(),
            },
        )
        .await?;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE,
            MyMessage {
                age: 25,
                name: "\newline encoded".into(),
            },
        )
        .await?;

    rabbit
        .publish_json(
            EXCHANGE,
            MESSAGE_TYPE_2,
            OtherMessage {
                school_age: SchoolAge::Primary,
                pupils: vec![
                    Pupil {
                        first_name: "jason".to_string(),
                        second_name: "mccullough".to_string(),
                    },
                    Pupil {
                        first_name: "david".to_string(),
                        second_name: "petran".to_string(),
                    },
                ],
            },
        )
        .await?;

    rabbit.close().await?;

    info!("goodbye!");

    Ok(())
}
