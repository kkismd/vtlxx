use std::io::{self, Read};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum InputEvent {
    Byte(u8),
    NotReady,
    Eof,
    Fatal,
}

pub(super) struct HostInput {
    prefix: std::io::Cursor<Vec<u8>>,
    terminal: Option<bool>, // true = EOF, false = fatal
    #[cfg(target_os = "linux")]
    live: Option<live::LiveInput>,
}

impl HostInput {
    pub(super) fn memory(bytes: Vec<u8>) -> Self {
        Self {
            prefix: std::io::Cursor::new(bytes),
            terminal: None,
            #[cfg(target_os = "linux")]
            live: None,
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn interactive(prefix: Vec<u8>, fd: std::os::fd::OwnedFd) -> Self {
        Self {
            prefix: std::io::Cursor::new(prefix),
            terminal: None,
            live: Some(live::LiveInput::new(fd)),
        }
    }

    fn next(&mut self, wait: bool) -> InputEvent {
        if let Some(eof) = self.terminal {
            return if eof {
                InputEvent::Eof
            } else {
                InputEvent::Fatal
            };
        }
        let mut byte = [0];
        if self.prefix.read(&mut byte).expect("memory read") != 0 {
            return InputEvent::Byte(byte[0]);
        }
        #[cfg(target_os = "linux")]
        let event = match &mut self.live {
            Some(live) => live.next(wait),
            None => InputEvent::Eof,
        };
        #[cfg(not(target_os = "linux"))]
        let event = {
            let _ = wait;
            InputEvent::Eof
        };
        match event {
            InputEvent::Eof => self.terminal = Some(true),
            InputEvent::Fatal => self.terminal = Some(false),
            _ => {}
        }
        event
    }

    pub(super) fn wait_byte(&mut self) -> InputEvent {
        self.next(true)
    }
    pub(super) fn try_byte(&mut self) -> InputEvent {
        self.next(false)
    }

    pub(super) fn read_for_pv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        match self.wait_byte() {
            InputEvent::Byte(byte) => {
                buffer[0] = byte;
                Ok(1)
            }
            InputEvent::Eof => Ok(0),
            InputEvent::Fatal => Err(io::Error::other("input worker failed")),
            InputEvent::NotReady => unreachable!(),
        }
    }

    pub(super) fn finish(&mut self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(live) = &mut self.live {
            return live.finish();
        }
        Ok(())
    }
}

impl Read for HostInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_for_pv(buffer)
    }
}

#[cfg(target_os = "linux")]
mod live {
    use super::InputEvent;
    use std::{
        collections::VecDeque,
        io::{self, Write},
        os::fd::{AsRawFd, OwnedFd},
        sync::{Arc, Condvar, Mutex},
        thread::{self, JoinHandle},
    };

    const CAPACITY: usize = 256;
    struct State {
        queue: VecDeque<InputEvent>,
        cancelled: bool,
        done: bool,
        failure: Option<String>,
    }
    struct Shared {
        state: Mutex<State>,
        changed: Condvar,
    }
    pub(super) struct LiveInput {
        fd: Option<OwnedFd>,
        shared: Arc<Shared>,
        wake: Option<io::PipeWriter>,
        worker: Option<JoinHandle<()>>,
    }
    impl LiveInput {
        pub(super) fn new(fd: OwnedFd) -> Self {
            Self {
                fd: Some(fd),
                shared: Arc::new(Shared {
                    state: Mutex::new(State {
                        queue: VecDeque::new(),
                        cancelled: false,
                        done: false,
                        failure: None,
                    }),
                    changed: Condvar::new(),
                }),
                wake: None,
                worker: None,
            }
        }
        fn start(&mut self) -> io::Result<()> {
            if self.fd.is_none() {
                return Ok(());
            }
            let (cancel, wake) = io::pipe()?;
            let fd = self.fd.take().unwrap();
            let shared = Arc::clone(&self.shared);
            self.worker = Some(thread::spawn(move || producer(fd, cancel, shared)));
            self.wake = Some(wake);
            Ok(())
        }
        pub(super) fn next(&mut self, wait: bool) -> InputEvent {
            if let Err(e) = self.start() {
                let mut state = self.shared.state.lock().unwrap();
                state.failure = Some(e.to_string());
                state.done = true;
                return InputEvent::Fatal;
            }
            let mut state = self.shared.state.lock().unwrap();
            loop {
                if let Some(event) = state.queue.pop_front() {
                    self.shared.changed.notify_all();
                    return event;
                }
                if state.done {
                    return InputEvent::Fatal;
                }
                if !wait {
                    return InputEvent::NotReady;
                }
                state = self.shared.changed.wait(state).unwrap();
            }
        }
        pub(super) fn finish(&mut self) -> io::Result<()> {
            {
                let mut state = self.shared.state.lock().unwrap();
                state.cancelled = true;
                self.shared.changed.notify_all();
            }
            if let Some(mut wake) = self.wake.take() {
                let _ = wake.write_all(&[1]);
            }
            if let Some(worker) = self.worker.take() {
                if worker.join().is_err() {
                    return Err(io::Error::other("input worker panicked"));
                }
            }
            let state = self.shared.state.lock().unwrap();
            if let Some(error) = &state.failure {
                return Err(io::Error::other(error.clone()));
            }
            Ok(())
        }
    }
    impl Drop for LiveInput {
        fn drop(&mut self) {
            let _ = self.finish();
        }
    }

    fn enqueue(shared: &Shared, event: InputEvent) -> bool {
        let mut state = shared.state.lock().unwrap();
        while state.queue.len() == CAPACITY && !state.cancelled {
            state = shared.changed.wait(state).unwrap();
        }
        if state.cancelled {
            return false;
        }
        state.queue.push_back(event);
        shared.changed.notify_all();
        true
    }
    fn producer(fd: OwnedFd, cancel: io::PipeReader, shared: Arc<Shared>) {
        let result = produce(&fd, &cancel, &shared);
        let mut state = shared.state.lock().unwrap();
        if let Err(e) = result {
            state.failure = Some(e.to_string());
            // Record the failure before attempting to enqueue it so finish sees it
            // even when cancellation interrupts a full queue.
            drop(state);
            enqueue(&shared, InputEvent::Fatal);
            state = shared.state.lock().unwrap();
        }
        state.done = true;
        shared.changed.notify_all();
    }
    fn produce(fd: &OwnedFd, cancel: &io::PipeReader, shared: &Shared) -> io::Result<()> {
        loop {
            let mut fds = [
                libc::pollfd {
                    fd: fd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: cancel.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let result = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
            if result < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if fds[0].revents & libc::POLLNVAL != 0 {
                return Err(io::Error::other("invalid input descriptor"));
            }
            if fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                let mut byte = [0u8];
                let n = unsafe { libc::read(fd.as_raw_fd(), byte.as_mut_ptr().cast(), 1) };
                if n < 0 {
                    let e = io::Error::last_os_error();
                    if e.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(e);
                }
                if n == 0 {
                    enqueue(shared, InputEvent::Eof);
                    return Ok(());
                }
                if !enqueue(shared, InputEvent::Byte(byte[0])) {
                    return Ok(());
                }
            }
            if fds[1].revents != 0 {
                return Ok(());
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn prefix_and_zero_length_read_do_not_start_worker() {
            let (reader, _writer) = io::pipe().unwrap();
            let mut input = super::super::HostInput::interactive(vec![1, 2], reader.into());
            assert_eq!(input.read_for_pv(&mut []).unwrap(), 0);
            assert_eq!(input.wait_byte(), InputEvent::Byte(1));
            assert_eq!(input.wait_byte(), InputEvent::Byte(2));
            assert!(input.live.as_ref().unwrap().worker.is_none());
            assert_eq!(input.try_byte(), InputEvent::NotReady);
            assert!(input.live.as_ref().unwrap().worker.is_some());
            input.finish().unwrap();
        }

        #[test]
        fn disconnected_producer_is_fatal_and_unconsumed_failure_is_reported() {
            let (reader, _writer) = io::pipe().unwrap();
            let mut input = LiveInput::new(reader.into());
            assert!(input.worker.is_none());
            // Simulate a producer that exits without a terminal event.
            input.fd.take();
            input.shared.state.lock().unwrap().done = true;
            assert_eq!(input.next(false), InputEvent::Fatal);
            input.shared.state.lock().unwrap().failure = Some("read failure".into());
            assert!(input.finish().is_err());
        }

        #[test]
        fn observed_failure_is_reported_even_without_consuming_terminal_event() {
            let directory: OwnedFd = std::fs::File::open(".").unwrap().into();
            let mut input = LiveInput::new(directory);
            input.start().unwrap();
            let mut state = input.shared.state.lock().unwrap();
            while !state.done {
                state = input.shared.changed.wait(state).unwrap();
            }
            assert!(state.failure.is_some());
            assert_eq!(state.queue.front(), Some(&InputEvent::Fatal));
            drop(state);
            assert!(input.finish().is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_paths_share_position_and_sticky_eof() {
        let mut input = HostInput::memory(vec![0, 255, 7]);
        assert_eq!(input.try_byte(), InputEvent::Byte(0));
        let mut bytes = [0; 2];
        assert_eq!(input.read_for_pv(&mut bytes).unwrap(), 1);
        assert_eq!(bytes[0], 255);
        assert_eq!(input.wait_byte(), InputEvent::Byte(7));
        assert_eq!(input.wait_byte(), InputEvent::Eof);
        assert_eq!(input.try_byte(), InputEvent::Eof);
        assert_eq!(input.read_for_pv(&mut bytes).unwrap(), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_pipe_delivers_fifo_eof_and_cancel_full_queue() {
        use std::{
            io::Write,
            time::{Duration, Instant},
        };
        let (reader, mut writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![42], reader.into());
        assert_eq!(input.try_byte(), InputEvent::Byte(42));
        assert_eq!(input.try_byte(), InputEvent::NotReady);
        writer.write_all(&[0, 255, 1]).unwrap();
        drop(writer);
        assert_eq!(input.wait_byte(), InputEvent::Byte(0));
        assert_eq!(input.wait_byte(), InputEvent::Byte(255));
        assert_eq!(input.wait_byte(), InputEvent::Byte(1));
        assert_eq!(input.wait_byte(), InputEvent::Eof);
        assert_eq!(input.try_byte(), InputEvent::Eof);
        input.finish().unwrap();

        let (reader, mut writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![], reader.into());
        input.try_byte();
        writer.write_all(&[1; 512]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let start = Instant::now();
        input.finish().unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pvread_waits_for_arrival_and_zero_length_does_not_start_worker() {
        use std::{
            io::Write,
            time::{Duration, Instant},
        };
        let (reader, mut writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![], reader.into());
        assert_eq!(input.read_for_pv(&mut []).unwrap(), 0);
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            writer.write_all(b"z").unwrap();
        });
        let start = Instant::now();
        let mut byte = [0];
        assert_eq!(input.read_for_pv(&mut byte).unwrap(), 1);
        assert!(start.elapsed() >= Duration::from_millis(75));
        assert_eq!(byte, *b"z");
        producer.join().unwrap();
        input.finish().unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn read_failure_is_fatal_and_finish_reports_observed_error() {
        use std::os::fd::OwnedFd;
        let directory: OwnedFd = std::fs::File::open(".").unwrap().into();
        let mut input = HostInput::interactive(vec![9], directory);
        assert_eq!(input.wait_byte(), InputEvent::Byte(9));
        assert_eq!(input.wait_byte(), InputEvent::Fatal);
        assert_eq!(input.wait_byte(), InputEvent::Fatal);
        assert!(input.finish().is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "process CPU measurement requires an isolated test run"]
    fn delayed_wait_blocks_without_busy_cpu() {
        use std::{
            io::Write,
            time::{Duration, Instant},
        };
        fn process_cpu() -> Duration {
            let mut ts = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            assert_eq!(
                unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) },
                0
            );
            Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
        }
        let (reader, mut writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![42], reader.into());
        assert_eq!(input.wait_byte(), InputEvent::Byte(42));
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(750));
            writer.write_all(b"x").unwrap();
        });
        let wall = Instant::now();
        let cpu = process_cpu();
        assert_eq!(input.wait_byte(), InputEvent::Byte(b'x'));
        assert!(wall.elapsed() >= Duration::from_millis(700));
        assert!(process_cpu() - cpu < Duration::from_millis(300));
        producer.join().unwrap();
        input.finish().unwrap();
    }
}
