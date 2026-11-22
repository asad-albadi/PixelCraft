//! Bounded background file availability checks. Never probe a filesystem on a UI frame.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Availability {
    Checking,
    Available,
    Missing,
}
struct Entry {
    available: Option<bool>,
    checked: Instant,
    pending: bool,
}
pub(super) struct RecentFiles {
    state: Arc<Mutex<HashMap<PathBuf, Entry>>>,
    jobs: mpsc::SyncSender<PathBuf>,
}
impl Default for RecentFiles {
    fn default() -> Self {
        Self::with_probe(Arc::new(|path| path.is_file()))
    }
}
impl RecentFiles {
    fn with_probe(probe: Arc<dyn Fn(&Path) -> bool + Send + Sync>) -> Self {
        let state = Arc::new(Mutex::new(HashMap::<PathBuf, Entry>::new()));
        let (jobs, rx) = mpsc::sync_channel::<PathBuf>(32);
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..2 {
            let (rx, state, probe) = (rx.clone(), state.clone(), probe.clone());
            std::thread::spawn(move || {
                loop {
                    let job = rx.lock().unwrap().recv();
                    let Ok(path) = job else {
                        break;
                    };
                    let available = probe(&path);
                    if let Some(entry) = state.lock().unwrap().get_mut(&path) {
                        entry.available = Some(available);
                        entry.checked = Instant::now();
                        entry.pending = false;
                    }
                }
            });
        }
        Self { state, jobs }
    }
    pub(super) fn availability(&self, path: &Path) -> Availability {
        let mut state = self.state.lock().unwrap();
        let entry = state.entry(path.to_path_buf()).or_insert(Entry {
            available: None,
            checked: Instant::now(),
            pending: false,
        });
        if !entry.pending
            && (entry.available.is_none() || entry.checked.elapsed() >= Duration::from_secs(5))
            && self.jobs.try_send(path.to_path_buf()).is_ok()
        {
            entry.pending = true;
        }
        match entry.available {
            Some(true) => Availability::Available,
            Some(false) => Availability::Missing,
            None => Availability::Checking,
        }
    }
    pub(super) fn pending(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .values()
            .any(|entry| entry.pending)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocked_probe_does_not_block_caller_or_spawn_unbounded_workers() {
        let (started, received) = mpsc::channel();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let probe_gate = gate.clone();
        let files = RecentFiles::with_probe(Arc::new(move |_| {
            let _ = started.send(());
            let (lock, cv) = &*probe_gate;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = cv.wait(released).unwrap();
            }
            false
        }));
        assert_eq!(
            files.availability(Path::new("first")),
            Availability::Checking
        );
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            files.availability(Path::new("second")),
            Availability::Checking
        );
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        for i in 0..100 {
            assert_eq!(
                files.availability(Path::new(&i.to_string())),
                Availability::Checking
            );
        }
        assert!(
            received.try_recv().is_err(),
            "only two workers may enter a blocked probe"
        );
        let (lock, cv) = &*gate;
        *lock.lock().unwrap() = true;
        cv.notify_all();
        // A worker completion updates the same cache used by menus and the picker.
        let deadline = Instant::now() + Duration::from_secs(5);
        while files.availability(Path::new("first")) == Availability::Checking {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            files.availability(Path::new("first")),
            Availability::Missing
        );
    }
}
