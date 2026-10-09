use prismer::Prism;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Speech {
    stop: Sender<()>,
    pub status: Receiver<String>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Speech {
    pub fn start(flags: Arc<AtomicU32>) -> Self {
        let (stop, rx) = mpsc::channel();
        let (status_tx, status) = mpsc::channel();
        let worker = thread::spawn(move || {
            let prism = match Prism::new() {
                Ok(p) => p,
                Err(e) => {
                    let _ = status_tx.send(format!("Speech unavailable: {e}"));
                    return;
                }
            };
            let mut last = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut last_status = String::new();
            loop {
                let time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let timeout = Duration::from_millis(100)
                    .min(Duration::from_secs(1) - Duration::from_nanos(time.subsec_nanos() as u64));
                if !matches!(
                    rx.recv_timeout(timeout),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    break;
                }
                let epoch = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if epoch == last {
                    continue;
                }
                last = epoch;
                if flags.load(Ordering::Relaxed) & 8 == 0 {
                    continue;
                }
                // Prismer prefers an active screen reader and falls back to a system voice.
                // Interrupting replaces an old seconds cue rather than building a speech backlog.
                let message = match prism.acquire_best() {
                    Ok(backend) => match backend.speak(&(epoch % 60).to_string(), true) {
                        Ok(()) => format!("Speech: {}", backend.name()),
                        Err(e) => format!("Speech unavailable: {e}"),
                    },
                    Err(e) => format!("Speech unavailable: {e}"),
                };
                if message != last_status {
                    let _ = status_tx.send(message.clone());
                    last_status = message;
                }
            }
        });
        Self {
            stop,
            status,
            worker: Some(worker),
        }
    }
}
impl Drop for Speech {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
