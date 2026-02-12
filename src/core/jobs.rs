use crate::core::CoreAsyncEvent;
use crate::core::consensus::{ConsensusMethod, compute_consensus};
use crate::core::data::SequenceRecord;
use crate::core::parser;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace, warn};

/// A consensus update request
#[derive(Debug, Clone)]
pub struct ConsensusRequest {
    pub sequences: std::sync::Arc<Vec<SequenceRecord>>,
    pub positions: Vec<usize>,
    pub method: ConsensusMethod,
}

/// Spawns an async task to parse alignments from a FASTA file.
///
/// On completion, sends `CoreAsyncEvent::AlignmentsLoaded` with either the
/// parsed alignments or an error message.
pub fn spawn_load_alignments_job(
    file_path: std::path::PathBuf,
    async_tx: mpsc::Sender<CoreAsyncEvent>,
) {
    tokio::spawn(async move {
        info!(path = ?file_path, "started alignment load job");
        let result = parser::parse_fasta_file(file_path).await;

        if let Err(send_error) = async_tx
            .send(CoreAsyncEvent::AlignmentsLoaded(
                result.map_err(|error| error.to_string()),
            ))
            .await
        {
            warn!(
                error = ?send_error,
                "failed to send alignment load event to app"
            );
        }
    });
}

/// Spawns a long-lived consensus worker task.
///
/// The worker listens for `ConsensusRequest` updates, cancels any ongoing
/// work when a newer request arrives
/// publishes completed updates as `CoreAsyncEvent::ConsensusUpdated`.
#[must_use]
pub fn spawn_consensus_worker(
    mut request_rx: watch::Receiver<Option<ConsensusRequest>>,
    async_tx: mpsc::Sender<CoreAsyncEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("consensus worker started");
        let mut active_cancel: Option<CancellationToken> = None;

        loop {
            if request_rx.changed().await.is_err() {
                debug!("consensus worker stopping because request channel closed");
                break;
            }

            let Some(request) = request_rx.borrow_and_update().clone() else {
                trace!("consensus worker received empty request");
                continue;
            };

            if let Some(cancel) = active_cancel.take() {
                trace!("cancelling previous consensus computation");
                cancel.cancel();
            }

            let cancel = CancellationToken::new();
            active_cancel = Some(cancel.clone());

            let ConsensusRequest {
                sequences,
                positions,
                method,
            } = request;
            trace!(
                sequence_count = sequences.len(),
                position_count = positions.len(),
                method = ?method,
                "consensus worker received request"
            );
            let blocking_cancel = cancel.clone();
            let updates = tokio::task::spawn_blocking(move || {
                compute_consensus(&sequences, &positions, method, &blocking_cancel)
            })
            .await
            .unwrap_or_else(|join_error| {
                warn!(
                    error = ?join_error,
                    "consensus computation task failed to join"
                );
                Vec::new()
            });

            if cancel.is_cancelled() {
                trace!("discarding consensus result because request was cancelled");
                continue;
            }

            if let Err(send_error) = async_tx
                .send(CoreAsyncEvent::ConsensusUpdated { updates })
                .await
            {
                warn!(
                    error = ?send_error,
                    "failed to send consensus update event to app"
                );
            }
        }

        info!("consensus worker stopped");
    })
}
