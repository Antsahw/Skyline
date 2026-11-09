//! One end of a connection to a Java runtime: synchronous calls that may nest.
//!
//! Horizon calls into Java (`tick`, `useBlock`, `report`); while Java runs that call, its mods
//! call back into Horizon (`blockState`, `light`, `setBlock`: what `Natives.java` is over JNI).
//! So the thread waiting for an answer keeps reading: a CALL that arrives meanwhile is
//! handled at once on the same thread (it may call back again), an EVENT is delivered, and
//! only the REPLY with its own id ends the wait. Both ends do this, so calls nest to any
//! depth in either direction, exactly as JNI calls do on one thread.
//!
//! One connection is one conversation: one thread on each side at a time
//! (`docs/java-bridge-120.md`, "Lanes"). Events are buffered and go out with the next call,
//! reply or [`Peer::flush`].

use crate::wire::{self, Frame, Value};
use std::io::{self, BufReader, BufWriter, Read, Write};

/// What failed.
#[derive(Debug)]
pub enum Error {
    /// The connection broke.
    Io(io::Error),
    /// The other side's handler failed (its message).
    Remote(String),
    /// The other side said goodbye, or closed the connection, while an answer was awaited.
    Closed,
    /// A frame that makes no sense here.
    Protocol(String),
    /// No frame came within the connection's read timeout.
    TimedOut,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "connection: {e}"),
            Error::Remote(m) => write!(f, "{m}"),
            Error::Closed => write!(f, "the Java runtime closed the connection"),
            Error::Protocol(m) => write!(f, "protocol: {m}"),
            Error::TimedOut => write!(f, "the Java runtime didn't answer in time"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

/// Answers the other side's calls and takes its events.
pub trait Handler {
    /// A call from the other side. May make calls of its own through `peer`.
    fn call(
        &mut self,
        peer: &mut Peer,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Vec<Value>, String>;

    /// A one-way event from the other side.
    fn event(&mut self, _peer: &mut Peer, _method: &str, _args: Vec<Value>) {}
}

/// Counters, for the benchmark and the log.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub calls_out: u64,
    pub calls_in: u64,
    pub events_out: u64,
    pub events_in: u64,
    pub bytes_out: u64,
    /// The deepest nesting seen (1 = a plain call).
    pub max_depth: u32,
}

/// What [`Peer::serve_one`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum Served {
    Call,
    Event,
    /// The other side said goodbye or closed the connection.
    Closed,
}

pub struct Peer {
    reader: BufReader<Box<dyn Read + Send>>,
    writer: BufWriter<Box<dyn Write + Send>>,
    next_id: u32,
    depth: u32,
    closed: bool,
    pub stats: Stats,
}

impl Peer {
    pub fn new(reader: Box<dyn Read + Send>, writer: Box<dyn Write + Send>) -> Peer {
        Peer {
            reader: BufReader::with_capacity(64 << 10, reader),
            writer: BufWriter::with_capacity(64 << 10, writer),
            next_id: 1,
            depth: 0,
            closed: false,
            stats: Stats::default(),
        }
    }

    fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        self.stats.bytes_out += frame.len() as u64;
        wire::write(&mut self.writer, frame)
    }

    /// Calls `method` on the other side and waits for its answer, handling the other side's
    /// calls and events meanwhile with `handler`.
    pub fn call(
        &mut self,
        method: &str,
        args: &[Value],
        handler: &mut dyn Handler,
    ) -> Result<Vec<Value>, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let frame = wire::call(id, method, args)?;
        self.send(&frame)?;
        self.writer.flush()?;
        self.stats.calls_out += 1;
        self.depth += 1;
        self.stats.max_depth = self.stats.max_depth.max(self.depth);
        let result = self.wait(id, handler);
        self.depth -= 1;
        result
    }

    fn wait(&mut self, id: u32, handler: &mut dyn Handler) -> Result<Vec<Value>, Error> {
        loop {
            match self.read()? {
                None | Some(Frame::Bye) => {
                    self.closed = true;
                    return Err(Error::Closed);
                }
                Some(Frame::Reply { id: got, values }) if got == id => return Ok(values),
                Some(Frame::Error { id: got, message }) if got == id => {
                    return Err(Error::Remote(message));
                }
                Some(Frame::Reply { id: got, .. } | Frame::Error { id: got, .. }) => {
                    return Err(Error::Protocol(format!(
                        "an answer to call {got} while waiting for {id}"
                    )));
                }
                Some(frame) => self.dispatch(frame, handler)?,
            }
        }
    }

    fn read(&mut self) -> Result<Option<Frame>, Error> {
        match wire::read(&mut self.reader) {
            Ok(frame) => Ok(frame),
            Err(e) => {
                // A timeout may land inside a frame: the stream can't be trusted after it.
                self.closed = true;
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) {
                    Err(Error::TimedOut)
                } else {
                    Err(Error::Io(e))
                }
            }
        }
    }

    fn dispatch(&mut self, frame: Frame, handler: &mut dyn Handler) -> Result<(), Error> {
        match frame {
            Frame::Call { id, method, args } => {
                self.stats.calls_in += 1;
                self.depth += 1;
                self.stats.max_depth = self.stats.max_depth.max(self.depth);
                let result = handler.call(self, &method, args);
                self.depth -= 1;
                let frame = match result {
                    Ok(values) => wire::reply(id, &values)?,
                    Err(message) => wire::error(id, &message)?,
                };
                self.send(&frame)?;
                self.writer.flush()?;
                Ok(())
            }
            Frame::Event { method, args } => {
                self.stats.events_in += 1;
                handler.event(self, &method, args);
                Ok(())
            }
            Frame::Hello { .. } => Err(Error::Protocol("a second hello".into())),
            other => Err(Error::Protocol(format!("unexpected {other:?}"))),
        }
    }

    /// Sends a one-way event (buffered until the next call, reply or [`Peer::flush`]).
    pub fn event(&mut self, method: &str, args: &[Value]) -> Result<(), Error> {
        let frame = wire::event(method, args)?;
        self.send(&frame)?;
        self.stats.events_out += 1;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush()?;
        Ok(())
    }

    /// Waits for one frame from the other side and handles it: for an idle side that
    /// expects calls or events (the Java runtime's main loop, or Horizon between ticks).
    pub fn serve_one(&mut self, handler: &mut dyn Handler) -> Result<Served, Error> {
        self.flush()?;
        match self.read()? {
            None | Some(Frame::Bye) => {
                self.closed = true;
                Ok(Served::Closed)
            }
            Some(frame @ Frame::Call { .. }) => {
                self.dispatch(frame, handler)?;
                Ok(Served::Call)
            }
            Some(frame @ Frame::Event { .. }) => {
                self.dispatch(frame, handler)?;
                Ok(Served::Event)
            }
            Some(other) => Err(Error::Protocol(format!("unexpected {other:?} while idle"))),
        }
    }

    /// Reads the child's hello (its first frame).
    pub fn read_hello(&mut self) -> Result<Vec<Value>, Error> {
        match self.read()? {
            Some(Frame::Hello { values }) => Ok(values),
            None => Err(Error::Closed),
            Some(other) => Err(Error::Protocol(format!("expected a hello, got {other:?}"))),
        }
    }

    /// Answers the hello (a reply to id 0).
    pub fn welcome(&mut self, values: &[Value]) -> Result<(), Error> {
        let frame = wire::reply(0, values)?;
        self.send(&frame)?;
        self.flush()
    }

    /// Says goodbye; the other side's serve loop ends.
    pub fn bye(&mut self) -> Result<(), Error> {
        if !self.closed {
            self.closed = true;
            let frame = wire::bye()?;
            self.send(&frame)?;
            self.flush()?;
        }
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    fn pair() -> (Peer, Peer) {
        let (a, b) = UnixStream::pair().unwrap();
        let peer = |s: UnixStream| Peer::new(Box::new(s.try_clone().unwrap()), Box::new(s));
        (peer(a), peer(b))
    }

    /// The "Java" side for the tests: `depth n` calls back `down n` into the caller, which
    /// calls `depth n-1` here again, so frames nest both ways; `fail` fails; `log` sends events.
    struct Far;
    impl Handler for Far {
        fn call(
            &mut self,
            peer: &mut Peer,
            method: &str,
            args: Vec<Value>,
        ) -> Result<Vec<Value>, String> {
            match method {
                "depth" => {
                    let n = args[0].as_i32().unwrap();
                    if n == 0 {
                        return Ok(vec![Value::Str("bottom".into())]);
                    }
                    let mut h = Far;
                    let got = peer
                        .call("down", &[Value::I32(n)], &mut h)
                        .map_err(|e| e.to_string())?;
                    Ok(vec![Value::Str(format!(
                        "{n}<{}",
                        got[0].as_str().unwrap()
                    ))])
                }
                "log" => {
                    for i in 0..3 {
                        peer.event("line", &[Value::I32(i)])
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(vec![])
                }
                "fail" => Err("no such block".into()),
                _ => Err(format!("unknown {method}")),
            }
        }
    }

    struct Near {
        lines: Vec<i32>,
    }
    impl Handler for Near {
        fn call(
            &mut self,
            peer: &mut Peer,
            method: &str,
            args: Vec<Value>,
        ) -> Result<Vec<Value>, String> {
            assert_eq!(method, "down");
            let n = args[0].as_i32().unwrap();
            let got = peer
                .call("depth", &[Value::I32(n - 1)], self)
                .map_err(|e| e.to_string())?;
            Ok(vec![Value::Str(format!("v{}", got[0].as_str().unwrap()))])
        }
        fn event(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) {
            assert_eq!(method, "line");
            self.lines.push(args[0].as_i32().unwrap());
        }
    }

    fn far_thread(mut far: Peer) -> std::thread::JoinHandle<()> {
        std::thread::spawn(
            move || {
                while far.serve_one(&mut Far).unwrap() != Served::Closed {}
            },
        )
    }

    #[test]
    fn calls_nest_both_ways_and_answers_find_their_caller() {
        let (mut near, far) = pair();
        let t = far_thread(far);
        let mut h = Near { lines: vec![] };
        let got = near.call("depth", &[Value::I32(3)], &mut h).unwrap();
        // depth 3 -> down 3 -> depth 2 -> down 2 -> depth 1 -> down 1 -> depth 0.
        assert_eq!(got, vec![Value::Str("3<v2<v1<vbottom".into())]);
        assert_eq!(near.stats.calls_out, 4);
        assert_eq!(near.stats.calls_in, 3);
        assert_eq!(near.stats.max_depth, 7);
        near.bye().unwrap();
        t.join().unwrap();
    }

    #[test]
    fn events_arrive_in_order_before_the_answer_and_failures_come_back_as_errors() {
        let (mut near, far) = pair();
        let t = far_thread(far);
        let mut h = Near { lines: vec![] };
        assert_eq!(near.call("log", &[], &mut h).unwrap(), vec![]);
        assert_eq!(h.lines, vec![0, 1, 2]);
        match near.call("fail", &[], &mut h) {
            Err(Error::Remote(m)) => assert_eq!(m, "no such block"),
            other => panic!("{other:?}"),
        }
        // The connection is still good after a failed call.
        assert_eq!(
            near.call("depth", &[Value::I32(0)], &mut h).unwrap(),
            vec![Value::Str("bottom".into())]
        );
        near.bye().unwrap();
        t.join().unwrap();
    }

    #[test]
    fn a_side_that_dies_mid_call_ends_the_wait() {
        let (mut near, far) = pair();
        drop(far);
        let mut h = Near { lines: vec![] };
        assert!(matches!(
            near.call("depth", &[Value::I32(1)], &mut h),
            Err(Error::Closed | Error::Io(_))
        ));
    }
}
