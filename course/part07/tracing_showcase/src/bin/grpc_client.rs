use clap::Parser;
use rand::Rng;
use tokio_util::task::TaskTracker;
use tower::ServiceBuilder;
use tracing::{info, info_span, instrument, Instrument};

use tracing_showcase::{
    grpc::proto::{cards_service_client::CardsServiceClient, DrawCardsRequest, NewDecksRequest},
    layers::{
        otlp_context_propagation::OtlpPropagatedTracingContextProducerLayer,
        request_counter::RequestCounterLayer,
    },
    tracing_setup::init_tracing,
};
use tokio::select;
use tokio::signal::ctrl_c;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 1)]
    loop_count: usize,

    #[arg(short, long, default_value_t = true, action = clap::ArgAction::Set)]
    drip_feed: bool,

    #[arg(short = 'm', long, default_value_t = 1)]
    min_parallelism: usize,

    #[arg(short = 'M', long, default_value_t = 1)]
    max_parallelism: usize,

    #[arg(short = 's', long, default_value_t = 10)]
    sleep_ms: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _handle = init_tracing("grpc caller")?;
    let mut args = Args::parse();

    if args.drip_feed && args.max_parallelism == 1 {
        args.max_parallelism = 10;
    }
    
    info!("Starting client arguments: loop_count={}, drip_feed={}, min_parallelism={}, max_parallelism={}, sleep_ms={}", 
        args.loop_count, args.drip_feed, args.min_parallelism, args.max_parallelism, args.sleep_ms);

    let mut i = 0;
    let mut shutdown = false;
    loop {
        if !args.drip_feed && i >= args.loop_count {
            break;
        }
        if shutdown {
            break;
        }

        info!("Start loop {}/{}", i + 1, if args.drip_feed { "infinity".to_string() } else { args.loop_count.to_string() });
        
        // Determine parallelism for this iteration
        let parallelism = if args.min_parallelism >= args.max_parallelism {
             args.min_parallelism
        } else {
             rand::thread_rng().gen_range(args.min_parallelism..=args.max_parallelism)
        };
        
        info!("Running with parallelism: {}", parallelism);
        
        let tracker = TaskTracker::new();
        for _ in 0..parallelism {
            tracker.spawn(async move {
                if let Err(e) = run_client().await {
                   tracing::error!("Client error: {}", e);
                }
            });
        }
        tracker.close();
        
        // Wait for all tasks to complete, but allow Ctrl+C to break after batch
        select! {
            _ = tracker.wait() => {},
            _ = ctrl_c() => {
                info!("Received Ctrl+C, will shutdown after this batch.");
                shutdown = true;
            }
        }

        i += 1;
        if shutdown {
            tracker.wait().await;
            break;
        }
    }

    info!("goodbye from the client!");

    Ok(())
}

#[instrument]
async fn run_client() -> anyhow::Result<()> {
    let url = std::env::var("GRPC_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:25565".to_string());
    let channel = tonic::transport::Endpoint::new(url)?
        .connect()
        .instrument(info_span!("connecting to server"))
        .await?;

    let client = tower::ServiceBuilder::new()
        .layer(
            ServiceBuilder::new()
                .layer(OtlpPropagatedTracingContextProducerLayer)
                .layer(RequestCounterLayer::new_for_http()),
        )
        .service(channel);
    let mut client = CardsServiceClient::new(client);

    let decks = client
        .new_decks(NewDecksRequest { decks: 5 })
        .instrument(info_span!("new decks request"))
        .await?
        .into_inner();

    let drawn_hands = client
        .draw_cards(DrawCardsRequest {
            deck_id: decks.deck_id.clone(),
            count: 5,
            hands: 20,
        })
        .instrument(info_span!("draw hands request"))
        .await?
        .into_inner();

    let cards = drawn_hands
        .hands
        .iter()
        .flat_map(|hand| hand.cards.iter())
        .count();

    info!("retrieved {cards} cards");

    Ok(())
}
