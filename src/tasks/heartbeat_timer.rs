use std::time::Duration;

use tokio::{
    sync::mpsc,
    time::{self, Instant},
};

use crate::core::events::Event;

pub enum HeartbeatTimerCmd {
    Start,
    Reset,
    Stop,
    Shutdown,
}

pub struct HeartbeatTimer {
    cmd_tx: mpsc::Sender<HeartbeatTimerCmd>,
    join: tokio::task::JoinHandle<()>,
}

impl HeartbeatTimer {
    pub fn spawn(event_tx: mpsc::Sender<Event>, interval_ms: u32) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<HeartbeatTimerCmd>(32);

        let interval = Duration::from_millis(interval_ms as u64);

        let join = tokio::spawn(async move {
            let mut deadline: Option<Instant> = None;

            let mut sleep = Box::pin(time::sleep_until(Instant::now()));

            loop {
                let armed = deadline.is_some();

                tokio::select! {
                    _ = &mut sleep, if armed => {
                        if event_tx.send(Event::HeartbeatTimerFired).await.is_err() {
                            break; // RIP core
                        }

                        let next = Instant::now() + interval;
                        deadline = Some(next);
                        sleep.as_mut().reset(next);
                    }

                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(HeartbeatTimerCmd::Start) => {
                                let next = Instant::now() + interval;
                                deadline = Some(next);
                                sleep.as_mut().reset(next);
                            }
                            Some(HeartbeatTimerCmd::Reset) => {
                                if deadline.is_some() {
                                    let next = Instant::now() + interval;
                                    deadline = Some(next);
                                    sleep.as_mut().reset(next);
                                }
                            }
                            Some(HeartbeatTimerCmd::Stop) => {
                                deadline = None;
                            }

                            Some(HeartbeatTimerCmd::Shutdown) | None => break
                        }
                    }
                }
            }
        });

        Self { cmd_tx, join }
    }

    pub async fn start(&self) -> Result<(), mpsc::error::SendError<HeartbeatTimerCmd>> {
        self.cmd_tx.send(HeartbeatTimerCmd::Start).await
    }

    pub async fn reset_deadline(&self) -> Result<(), mpsc::error::SendError<HeartbeatTimerCmd>> {
        self.cmd_tx.send(HeartbeatTimerCmd::Reset).await
    }

    pub async fn stop(&self) {
        let _ = self.cmd_tx.send(HeartbeatTimerCmd::Stop).await;
    }

    pub async fn shutdown(self) {
        let _ = self.cmd_tx.send(HeartbeatTimerCmd::Shutdown).await;
        let _ = self.join.await;
    }
}
