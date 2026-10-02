//! Registry commands that answer later. `agent.connection`, `agent.models`,
//! `generate.audio` and `generate.place` start a live job (the network, a decode) and reply
//! "running"; the panel waits for the real answer without holding the interface thread.

use crate::{control::Reply, ui::daw::Daw};
use gpui::{App, Entity};
use ryolune_engine::Result;
use serde_json::Value;
use std::{future::Future, sync::mpsc, time::Duration};

/// How often a waiting request looks for its answer.
const POLL: Duration = Duration::from_millis(50);

/// Run `method` now and resolve with its result: at once for an ordinary command, when the
/// job finishes for one that runs in the background. Errors come back to the caller, which
/// shows them in place (no error dialog).
pub fn request(
    daw: &Entity<Daw>,
    method: &str,
    params: Value,
    cx: &mut App,
) -> impl Future<Output = Result<Value>> + 'static {
    let (tx, rx) = mpsc::sync_channel(1);
    let immediate = daw.update(cx, |daw, cx| match daw.request(method, params, cx) {
        Ok(value) if value["status"] == "running" => {
            match daw.app.attach_reply(Reply::Channel(tx)) {
                Ok(()) => None,
                // Nothing to wait for after all: the answer is the one we have.
                Err(_) => Some(Ok(value)),
            }
        }
        other => Some(other),
    });
    let executor = cx.background_executor().clone();
    async move {
        if let Some(result) = immediate {
            return result;
        }
        loop {
            match rx.try_recv() {
                Ok(result) => return result,
                Err(mpsc::TryRecvError::Empty) => executor.timer(POLL).await,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("The request stopped before it answered".into())
                }
            }
        }
    }
}
