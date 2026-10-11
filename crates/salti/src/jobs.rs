use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{debug, error};

use crate::{
    core::parser,
    update::{self, UpdateResult},
};

#[derive(Debug)]
pub struct Jobs {
    tx: UnboundedSender<Done>,
    rx: UnboundedReceiver<Done>,
    all: CancellationToken,
    load: Option<DropGuard>,
}

impl Jobs {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            tx,
            rx,
            all: CancellationToken::new(),
            load: None,
        }
    }

    pub fn load(&mut self, input: String) {
        let cancel = self.all.child_token();
        self.load = Some(cancel.clone().drop_guard());
        let token = cancel.clone();
        self.spawn(cancel, async move {
            let parse = {
                let input = input.clone();
                move || parser::parse_fasta_file(&input, &token)
            };
            let result = match tokio::task::spawn_blocking(parse).await {
                Ok(result) => result.map_err(|error| error.to_string()),
                Err(error) => {
                    error!(error = ?error, "Alignment load panicked");
                    Err("Loading the alignment failed unexpectedly".to_string())
                }
            };
            JobEvent::Loaded { input, result }
        });
    }

    pub fn check_for_update(&self, requested: bool) {
        self.spawn(self.all.child_token(), async move {
            let result = update::check_for_update()
                .await
                .inspect_err(|error| debug!(error = ?error, "Update check failed"))
                .ok();
            JobEvent::UpdateChecked { result, requested }
        });
    }

    pub async fn next(&mut self) -> JobEvent {
        loop {
            let done = self.rx.recv().await.expect("jobs holds a sender");
            if !done.cancel.is_cancelled() {
                return done.event;
            }
        }
    }

    fn spawn(
        &self,
        cancel: CancellationToken,
        work: impl Future<Output = JobEvent> + Send + 'static,
    ) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = work.await;
            let _ = tx.send(Done { cancel, event });
        });
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        self.all.cancel();
    }
}

#[derive(Debug)]
pub enum JobEvent {
    Loaded {
        input: String,
        result: Result<libmsa::Alignment, String>,
    },
    UpdateChecked {
        result: Option<UpdateResult>,
        requested: bool,
    },
}

#[derive(Debug)]
struct Done {
    cancel: CancellationToken,
    event: JobEvent,
}
