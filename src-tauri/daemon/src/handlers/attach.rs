use std::sync::atomic::Ordering;

use godly_protocol::{DaemonMessage, Event, Response};

use crate::debug_log::daemon_log;

use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    let sessions_guard = ctx.sessions.read();
    match sessions_guard.get(session_id) {
        Some(session) => {
            let is_already_dead = !session.is_running();
            let (buffer, mut rx) = session.attach();
            ctx.attached_sessions.write().push(session_id.to_string());

            // Spawn a task to forward live output as events.
            // When the channel closes, check if the PTY exited (running == false)
            // and send SessionClosed so the client knows the process is dead.
            let tx = ctx.msg_tx.clone();
            let sid = session_id.to_string();
            let running_flag = session.running_flag();
            let session_exit_code = session.exit_code();
            let exit_code_arc = session.exit_code_arc();
            tokio::spawn(async move {
                // Bug A2 fix: if the session is already dead when we attach,
                // send SessionClosed immediately. The reader thread and child
                // monitor are already gone, so the output channel will never
                // close on its own — rx.recv() would block forever.
                if is_already_dead {
                    daemon_log!(
                        "Session {} already dead at attach time, sending SessionClosed (exit_code={:?})",
                        sid,
                        session_exit_code
                    );
                    let _ = tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: session_exit_code,
                        }))
                        .await;
                    return;
                }

                while let Some(output) = rx.recv().await {
                    let event = match output {
                        crate::session::SessionOutput::RawBytes(data) => {
                            DaemonMessage::Event(Event::Output {
                                session_id: sid.clone(),
                                data,
                            })
                        }
                        crate::session::SessionOutput::GridDiff(diff) => {
                            DaemonMessage::Event(Event::GridDiff {
                                session_id: sid.clone(),
                                diff,
                            })
                        }
                        crate::session::SessionOutput::Bell => {
                            DaemonMessage::Event(Event::Bell {
                                session_id: sid.clone(),
                            })
                        }
                    };
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
                // Channel closed — check why:
                // - running == false → PTY exited → notify client
                // - running == true  → client detached → session still alive, don't notify
                if !running_flag.load(Ordering::Relaxed) {
                    let code = {
                        let raw = exit_code_arc.load(Ordering::Relaxed);
                        if raw == i64::MIN { None } else { Some(raw) }
                    };
                    daemon_log!("Session {} PTY exited, sending SessionClosed (exit_code={:?})", sid, code);
                    let _ = tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: code,
                        }))
                        .await;
                }
            });

            eprintln!(
                "[daemon] Attached to session {} (buffer: {} bytes)",
                session_id,
                buffer.len()
            );
            daemon_log!(
                "Attached to session {} (buffer: {} bytes)",
                session_id,
                buffer.len()
            );

            // Return buffered data for replay
            if buffer.is_empty() {
                Response::Ok
            } else {
                Response::Buffer {
                    session_id: session_id.to_string(),
                    data: buffer,
                }
            }
        }
        None => Response::Error {
            message: format!("Session {} not found", session_id),
        },
    }
}
