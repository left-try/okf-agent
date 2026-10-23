use notify::{RecursiveMode, Watcher};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Guard {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start(root: PathBuf) -> Guard {
    let (events_tx, events_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();
    let thread = thread::spawn(move || {
        let callback_tx = events_tx.clone();
        let Ok(mut watcher) = notify::recommended_watcher(move |event| {
            let _ = callback_tx.send(event);
        }) else {
            return;
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }
        let mut last_reconcile = Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match events_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(_)) => {
                    thread::sleep(Duration::from_millis(300));
                    while events_rx.try_recv().is_ok() {}
                    let _ = okf_application::repository_index::build(
                        &root,
                        &okf_adapter_fs::discovery::FsSourceCatalog,
                        &okf_adapter_sqlite::repository::SqliteIndexStore,
                    );
                    last_reconcile = Instant::now();
                }
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout)
                    if last_reconcile.elapsed() >= Duration::from_secs(30) =>
                {
                    let _ = okf_application::repository_index::build(
                        &root,
                        &okf_adapter_fs::discovery::FsSourceCatalog,
                        &okf_adapter_sqlite::repository::SqliteIndexStore,
                    );
                    last_reconcile = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Guard {
        stop: stop_tx,
        thread: Some(thread),
    }
}
