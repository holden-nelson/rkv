use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use crate::core::events::Event;

pub enum ElectionTimerCmd {
    ResetDeadline(Instant),
    Stop,
}

pub struct ElectionTimer {
    cmd_tx: mpsc::Sender<ElectionTimerCmd>,
    join: tokio::task::JoinHandle<()>,
}

impl ElectionTimer {
    pub fn spawn(event_tx: mpsc::Sender<Event>, initial_deadline: Instant) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ElectionTimerCmd>(32);

        let join = tokio::spawn(async move {
            let mut deadline: Option<Instant> = Some(initial_deadline);

            let mut sleep = Box::pin(time::sleep_until(deadline.unwrap()));

            loop {
                let armed = deadline.is_some();

                tokio::select! {
                    _ = &mut sleep, if armed => {
                        if event_tx.send(Event::ElectionTimeoutFired).await.is_err() {
                            break; // RIP core
                        }
                        deadline = None;
                    }

                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(ElectionTimerCmd::ResetDeadline(d)) => {
                                deadline = Some(d);
                                sleep.as_mut().reset(d);
                            }
                            Some(ElectionTimerCmd::Stop) | None => break
                        }
                    }
                }
            }
        });

        Self { cmd_tx, join }
    }

    pub async fn reset_deadline(
        &self,
        deadline: Instant,
    ) -> Result<(), mpsc::error::SendError<ElectionTimerCmd>> {
        self.cmd_tx
            .send(ElectionTimerCmd::ResetDeadline(deadline))
            .await
    }

    pub async fn stop(self) {
        let _ = self.cmd_tx.send(ElectionTimerCmd::Stop).await;
        let _ = self.join.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio::time::{self, Duration};

    fn far_future_deadline() -> Instant {
        Instant::now() + Duration::from_secs(60 * 60)
    }

    async fn expect_no_event(rx: &mut mpsc::Receiver<Event>) {
        for _ in 0..10 {
            match rx.try_recv() {
                Ok(_) => panic!("expected no event, but received one"),
                Err(TryRecvError::Empty) => tokio::task::yield_now().await,
                Err(TryRecvError::Disconnected) => return,
            }
        }
    }

    async fn expect_event(rx: &mut mpsc::Receiver<Event>) -> Event {
        for _ in 0..50 {
            match rx.try_recv() {
                Ok(e) => return e,
                Err(TryRecvError::Empty) => tokio::task::yield_now().await,
                Err(TryRecvError::Disconnected) => panic!("event channel disconnected"),
            }
        }
        panic!("timed out waiting for event");
    }

    #[tokio::test(start_paused = true)]
    async fn fires_once_at_deadline_and_then_disarms() {
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(16);
        let timer = ElectionTimer::spawn(event_tx, far_future_deadline());

        let deadline = Instant::now() + Duration::from_secs(10);
        timer.reset_deadline(deadline).await.unwrap();

        time::advance(Duration::from_secs(9)).await;
        expect_no_event(&mut event_rx).await;

        time::advance(Duration::from_secs(1)).await;
        let e = expect_event(&mut event_rx).await;
        assert!(matches!(e, Event::ElectionTimeoutFired));

        // After firing, it should be disarmed until reset again.
        time::advance(Duration::from_secs(60)).await;
        expect_no_event(&mut event_rx).await;

        timer.stop().await;
    }

    #[tokio::test(start_paused = true)]
    async fn reset_to_later_deadline_delays_firing() {
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(16);
        let timer = ElectionTimer::spawn(event_tx, far_future_deadline());

        let t0 = Instant::now();
        timer
            .reset_deadline(t0 + Duration::from_secs(5))
            .await
            .unwrap();

        // After 1s, push the deadline out to 10s from t0.
        time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;

        timer
            .reset_deadline(t0 + Duration::from_secs(10))
            .await
            .unwrap();

        // Move to just before 10s: should not have fired.
        time::advance(Duration::from_secs(8)).await; // total 9s
        expect_no_event(&mut event_rx).await;

        // Cross 10s: should fire once.
        time::advance(Duration::from_secs(1)).await; // total 10s
        let e = expect_event(&mut event_rx).await;
        assert!(matches!(e, Event::ElectionTimeoutFired));

        timer.stop().await;
    }

    #[tokio::test(start_paused = true)]
    async fn reset_to_earlier_deadline_fires_sooner() {
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(16);
        let timer = ElectionTimer::spawn(event_tx, far_future_deadline());

        let t0 = Instant::now();
        timer
            .reset_deadline(t0 + Duration::from_secs(20))
            .await
            .unwrap();

        // After 1s, pull deadline in to 3s from t0.
        time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;

        timer
            .reset_deadline(t0 + Duration::from_secs(3))
            .await
            .unwrap();

        // Move to just before 3s: should not have fired.
        time::advance(Duration::from_secs(1)).await; // total 2s
        expect_no_event(&mut event_rx).await;

        // Cross 3s: should fire.
        time::advance(Duration::from_secs(1)).await; // total 3s
        let e = expect_event(&mut event_rx).await;
        assert!(matches!(e, Event::ElectionTimeoutFired));

        timer.stop().await;
    }

    #[tokio::test(start_paused = true)]
    async fn stop_exits_cleanly_when_armed() {
        let (event_tx, _event_rx) = mpsc::channel::<Event>(1);
        let timer = ElectionTimer::spawn(event_tx, far_future_deadline());

        // Should exit promptly even when armed (deadline far in the future).
        timer.stop().await;
    }

    #[tokio::test(start_paused = true)]
    async fn initial_deadline_fires_without_reset() {
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(16);

        let deadline = Instant::now() + Duration::from_secs(5);
        let timer = ElectionTimer::spawn(event_tx, deadline);

        time::advance(Duration::from_secs(4)).await;
        expect_no_event(&mut event_rx).await;

        time::advance(Duration::from_secs(1)).await;
        let e = expect_event(&mut event_rx).await;
        assert!(matches!(e, Event::ElectionTimeoutFired));

        // After firing, it should be disarmed until reset again.
        time::advance(Duration::from_secs(60)).await;
        expect_no_event(&mut event_rx).await;

        timer.stop().await;
    }
}
