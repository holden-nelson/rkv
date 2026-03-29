use crate::context::NodeContext;
use crate::{core::events::Event, tasks::timer::ElectionTimer};

use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

pub async fn run(ctx: &NodeContext) {
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(128);

    let election_timeout = get_next_timeout_deadline(ctx);
    let timer = ElectionTimer::spawn(event_tx, election_timeout);

    while let Some(event) = event_rx.recv().await {
        match event {
            Event::ElectionTimeoutFired => {
                println!("election timed out!");

                let _ = timer.reset_deadline(get_next_timeout_deadline(ctx)).await;
            }
        }
    }

    timer.stop().await;
}

fn get_next_timeout_deadline(ctx: &NodeContext) -> Instant {
    Instant::now() + Duration::from_millis(ctx.election_timeout_ms.into())
}
