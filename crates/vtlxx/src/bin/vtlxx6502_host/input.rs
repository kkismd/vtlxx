use std::io::{self, Read};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum InputEvent {
    Byte(u8),
    NotReady,
    Eof,
    Fatal,
    Interrupted,
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

    pub(super) fn finish(&mut self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(live) = &mut self.live {
            return live.finish();
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(super) fn set_signal_fd(&mut self, fd: std::os::fd::RawFd) -> io::Result<()> {
        if let Some(live) = &mut self.live {
            live.set_signal_fd(fd)?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod live {
    use super::InputEvent;
    use std::{
        collections::VecDeque,
        io::{self, Write},
        os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        sync::{Arc, Condvar, Mutex},
        thread::{self, JoinHandle},
    };

    const CAPACITY: usize = 256;
    struct State {
        queue: VecDeque<InputEvent>,
        cancelled: bool,
        done: bool,
        failure: Option<String>,
        interrupted: bool,
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
        signal: Option<OwnedFd>,
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
                        interrupted: false,
                    }),
                    changed: Condvar::new(),
                }),
                wake: None,
                worker: None,
                signal: None,
            }
        }
        pub(super) fn set_signal_fd(&mut self, fd: RawFd) -> io::Result<()> {
            let copy = unsafe { libc::dup(fd) };
            if copy < 0 {
                return Err(io::Error::last_os_error());
            }
            self.signal = Some(unsafe { OwnedFd::from_raw_fd(copy) });
            Ok(())
        }
        fn start(&mut self) -> io::Result<()> {
            if self.fd.is_none() {
                return Ok(());
            }
            let (cancel, wake) = io::pipe()?;
            let fd = self.fd.take().unwrap();
            let shared = Arc::clone(&self.shared);
            let signal = self.signal.take();
            self.worker = Some(thread::spawn(move || producer(fd, cancel, signal, shared)));
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
                if state.interrupted {
                    state.interrupted = false;
                    self.shared.changed.notify_all();
                    return InputEvent::Interrupted;
                }
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
            if let Some(worker) = self.worker.take()
                && worker.join().is_err()
            {
                return Err(io::Error::other("input worker panicked"));
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
    fn producer(fd: OwnedFd, cancel: io::PipeReader, signal: Option<OwnedFd>, shared: Arc<Shared>) {
        let result = produce(&fd, &cancel, signal.as_ref(), &shared);
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
    fn produce(
        fd: &OwnedFd,
        cancel: &io::PipeReader,
        signal: Option<&OwnedFd>,
        shared: &Shared,
    ) -> io::Result<()> {
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
                libc::pollfd {
                    fd: signal.map_or(-1, AsRawFd::as_raw_fd),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let result = unsafe { libc::poll(fds.as_mut_ptr(), 3, -1) };
            if result < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if fds[2].revents != 0 {
                let mut state = shared.state.lock().unwrap();
                state.interrupted = true;
                shared.changed.notify_all();
                while state.interrupted && !state.cancelled {
                    state = shared.changed.wait(state).unwrap();
                }
                if state.cancelled {
                    return Ok(());
                }
                continue;
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
        fn prefix_does_not_start_worker() {
            let (reader, _writer) = io::pipe().unwrap();
            let mut input = super::super::HostInput::interactive(vec![1, 2], reader.into());
            assert_eq!(input.try_byte(), InputEvent::Byte(1));
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
        assert_eq!(input.wait_byte(), InputEvent::Byte(255));
        assert_eq!(input.wait_byte(), InputEvent::Byte(7));
        assert_eq!(input.wait_byte(), InputEvent::Eof);
        assert_eq!(input.try_byte(), InputEvent::Eof);
        assert_eq!(input.wait_byte(), InputEvent::Eof);
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
    fn wait_waits_for_arrival() {
        use std::{
            io::Write,
            time::{Duration, Instant},
        };
        let (reader, mut writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![], reader.into());
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            writer.write_all(b"z").unwrap();
        });
        let start = Instant::now();
        assert_eq!(input.wait_byte(), InputEvent::Byte(b'z'));
        assert!(start.elapsed() >= Duration::from_millis(75));
        producer.join().unwrap();
        input.finish().unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn signal_wakes_blocked_input_without_becoming_guest_data() {
        use std::{
            io::{Read, Write},
            os::fd::AsRawFd,
            sync::mpsc,
            time::Duration,
        };
        let (reader, mut writer) = io::pipe().unwrap();
        let (mut signal_reader, mut signal_writer) = io::pipe().unwrap();
        let mut input = HostInput::interactive(vec![], reader.into());
        input.set_signal_fd(signal_reader.as_raw_fd()).unwrap();
        let (send, receive) = mpsc::channel();
        let (resume, resumed) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            send.send(input.wait_byte()).unwrap();
            resumed.recv().unwrap();
            let event = loop {
                let event = input.wait_byte();
                if event != InputEvent::Interrupted {
                    break event;
                }
            };
            input.finish().unwrap();
            event
        });
        signal_writer.write_all(&[1]).unwrap();
        assert_eq!(
            receive.recv_timeout(Duration::from_secs(2)).unwrap(),
            InputEvent::Interrupted
        );
        signal_reader.read_exact(&mut [0]).unwrap();
        resume.send(()).unwrap();
        writer.write_all(b"x").unwrap();
        assert_eq!(thread.join().unwrap(), InputEvent::Byte(b'x'));
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
