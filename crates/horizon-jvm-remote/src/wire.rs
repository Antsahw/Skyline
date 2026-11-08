//! The frames between Horizon and a Java runtime in a process of its own
//! (`docs/java-bridge-120.md`, "The transport"). The Java side is
//! `java/dev/horizonlab/bridge/remote/Wire.java`; both read and write exactly this:
//!
//! ```text
//! frame  := len:u32  kind:u8  id:u32  body          (len counts kind, id and body; little endian)
//! CALL   := method:str  count:u16  value*           (an answer is expected, under the same id)
//! EVENT  := method:str  count:u16  value*           (one way, id 0)
//! REPLY  := count:u16  value*
//! ERROR  := message:str                             (the call failed on the other side)
//! HELLO  := count:u16  value*                       (the child's first frame: protocol, token, ...)
//! BYE    := (empty)                                 (close cleanly)
//! str    := len:u32  utf8
//! value  := tag:u8  payload                         (see [`Value`])
//! ```
//!
//! The values are the ones `Natives.java` passes over JNI today (strings, SNBT and JSON as
//! strings, numbers, byte and int arrays, string arrays), so its methods map one to one.

use std::io::{self, Read, Write};

/// The largest frame either side accepts: a corrupt length fails at once instead of
/// allocating gigabytes.
pub const MAX_FRAME: usize = 256 << 20;

/// What a frame is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Call = 1,
    Reply = 2,
    Error = 3,
    Event = 4,
    Hello = 5,
    Bye = 6,
}

impl Kind {
    fn from_u8(b: u8) -> Option<Kind> {
        Some(match b {
            1 => Kind::Call,
            2 => Kind::Reply,
            3 => Kind::Error,
            4 => Kind::Event,
            5 => Kind::Hello,
            6 => Kind::Bye,
            _ => return None,
        })
    }
}

/// One argument or result.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(String),
    Bytes(Vec<u8>),
    I32s(Vec<i32>),
    Strs(Vec<String>),
}

impl Value {
    fn tag(&self) -> u8 {
        match self {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::I32(_) => 2,
            Value::I64(_) => 3,
            Value::F32(_) => 4,
            Value::F64(_) => 5,
            Value::Str(_) => 6,
            Value::Bytes(_) => 7,
            Value::I32s(_) => 8,
            Value::Strs(_) => 9,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::I64(v) => Some(*v),
            Value::I32(v) => Some(i64::from(*v)),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Value {
        Value::I32(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Value {
        Value::I64(v)
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Value {
        Value::Bool(v)
    }
}

/// A frame read off the connection.
#[derive(Debug, PartialEq)]
pub enum Frame {
    Call {
        id: u32,
        method: String,
        args: Vec<Value>,
    },
    Event {
        method: String,
        args: Vec<Value>,
    },
    Reply {
        id: u32,
        values: Vec<Value>,
    },
    Error {
        id: u32,
        message: String,
    },
    Hello {
        values: Vec<Value>,
    },
    Bye,
}

/// Builds one frame's bytes; [`Encoder::finish`] fills in the length.
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    pub fn new(kind: Kind, id: u32) -> Encoder {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&[0; 4]);
        buf.push(kind as u8);
        buf.extend_from_slice(&id.to_le_bytes());
        Encoder { buf }
    }

    pub fn str(&mut self, s: &str) -> &mut Self {
        self.u32(s.len() as u32);
        self.buf.extend_from_slice(s.as_bytes());
        self
    }

    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn values(&mut self, values: &[Value]) -> io::Result<&mut Self> {
        let count = u16::try_from(values.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "more than 65535 values"))?;
        self.buf.extend_from_slice(&count.to_le_bytes());
        for value in values {
            self.value(value);
        }
        Ok(self)
    }

    fn value(&mut self, value: &Value) {
        self.buf.push(value.tag());
        match value {
            Value::Null => {}
            Value::Bool(b) => self.buf.push(u8::from(*b)),
            Value::I32(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
            Value::I64(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
            Value::F32(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
            Value::F64(v) => self.buf.extend_from_slice(&v.to_le_bytes()),
            Value::Str(s) => {
                self.str(s);
            }
            Value::Bytes(b) => {
                self.u32(b.len() as u32);
                self.buf.extend_from_slice(b);
            }
            Value::I32s(a) => {
                self.u32(a.len() as u32);
                for v in a {
                    self.buf.extend_from_slice(&v.to_le_bytes());
                }
            }
            Value::Strs(a) => {
                self.u32(a.len() as u32);
                for s in a {
                    self.str(s);
                }
            }
        }
    }

    /// The frame, length filled in.
    pub fn finish(mut self) -> io::Result<Vec<u8>> {
        let len = self.buf.len() - 4;
        if len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("frame of {len} bytes is over the limit"),
            ));
        }
        self.buf[..4].copy_from_slice(&(len as u32).to_le_bytes());
        Ok(self.buf)
    }
}

/// A call frame's bytes.
pub fn call(id: u32, method: &str, args: &[Value]) -> io::Result<Vec<u8>> {
    let mut e = Encoder::new(Kind::Call, id);
    e.str(method).values(args)?;
    e.finish()
}

/// An event frame's bytes.
pub fn event(method: &str, args: &[Value]) -> io::Result<Vec<u8>> {
    let mut e = Encoder::new(Kind::Event, 0);
    e.str(method).values(args)?;
    e.finish()
}

/// A reply frame's bytes.
pub fn reply(id: u32, values: &[Value]) -> io::Result<Vec<u8>> {
    let mut e = Encoder::new(Kind::Reply, id);
    e.values(values)?;
    e.finish()
}

/// An error frame's bytes.
pub fn error(id: u32, message: &str) -> io::Result<Vec<u8>> {
    let mut e = Encoder::new(Kind::Error, id);
    e.str(message);
    e.finish()
}

/// A hello frame's bytes.
pub fn hello(values: &[Value]) -> io::Result<Vec<u8>> {
    let mut e = Encoder::new(Kind::Hello, 0);
    e.values(values)?;
    e.finish()
}

/// A bye frame's bytes.
pub fn bye() -> io::Result<Vec<u8>> {
    Encoder::new(Kind::Bye, 0).finish()
}

fn bad(what: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.into())
}

/// Reads one frame. `Ok(None)` is a clean end of stream between frames.
pub fn read(r: &mut impl Read) -> io::Result<Option<Frame>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len) as usize;
    if !(5..=MAX_FRAME).contains(&len) {
        return Err(bad(format!("frame length {len}")));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    let mut d = Decoder { buf: &body, pos: 5 };
    let kind = Kind::from_u8(body[0]).ok_or_else(|| bad(format!("frame kind {}", body[0])))?;
    let id = u32::from_le_bytes([body[1], body[2], body[3], body[4]]);
    let frame = match kind {
        Kind::Call => Frame::Call {
            id,
            method: d.str()?,
            args: d.values()?,
        },
        Kind::Event => Frame::Event {
            method: d.str()?,
            args: d.values()?,
        },
        Kind::Reply => Frame::Reply {
            id,
            values: d.values()?,
        },
        Kind::Error => Frame::Error {
            id,
            message: d.str()?,
        },
        Kind::Hello => Frame::Hello {
            values: d.values()?,
        },
        Kind::Bye => Frame::Bye,
    };
    if d.pos != body.len() {
        return Err(bad(format!(
            "{} bytes left over in a {kind:?} frame",
            body.len() - d.pos
        )));
    }
    Ok(Some(frame))
}

/// Writes a frame's bytes (not flushed).
pub fn write(w: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    w.write_all(frame)
}

struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Decoder<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.buf.len())
            .ok_or_else(|| bad("frame ends inside a value"))?;
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn str(&mut self) -> io::Result<String> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| bad("string is not UTF-8"))
    }

    fn values(&mut self) -> io::Result<Vec<Value>> {
        let count = u16::from_le_bytes(self.array()?) as usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.value()?);
        }
        Ok(out)
    }

    fn value(&mut self) -> io::Result<Value> {
        let tag = self.array::<1>()?[0];
        Ok(match tag {
            0 => Value::Null,
            1 => Value::Bool(self.array::<1>()?[0] != 0),
            2 => Value::I32(i32::from_le_bytes(self.array()?)),
            3 => Value::I64(i64::from_le_bytes(self.array()?)),
            4 => Value::F32(f32::from_le_bytes(self.array()?)),
            5 => Value::F64(f64::from_le_bytes(self.array()?)),
            6 => Value::Str(self.str()?),
            7 => {
                let len = self.u32()? as usize;
                Value::Bytes(self.take(len)?.to_vec())
            }
            8 => {
                let len = self.u32()? as usize;
                let bytes = self.take(len.checked_mul(4).ok_or_else(|| bad("int array"))?)?;
                Value::I32s(
                    bytes
                        .chunks_exact(4)
                        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect(),
                )
            }
            9 => {
                let len = self.u32()? as usize;
                if len > self.buf.len() {
                    return Err(bad("string array longer than its frame"));
                }
                let mut out = Vec::with_capacity(len);
                for _ in 0..len {
                    out.push(self.str()?);
                }
                Value::Strs(out)
            }
            t => return Err(bad(format!("value tag {t}"))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_values() -> Vec<Value> {
        vec![
            Value::Null,
            Value::Bool(true),
            Value::I32(-7),
            Value::I64(1 << 40),
            Value::F32(0.5),
            Value::F64(-2.25),
            Value::Str("minecraft:stone[ä]".into()),
            Value::Bytes(vec![0, 255, 3]),
            Value::I32s(vec![1, -1, i32::MAX]),
            Value::Strs(vec!["a".into(), String::new()]),
        ]
    }

    #[test]
    fn every_value_and_kind_reads_back() {
        let mut stream = Vec::new();
        stream.extend(call(9, "blockState", &all_values()).unwrap());
        stream.extend(event("log", &["hi".into()]).unwrap());
        stream.extend(reply(9, &[Value::Str("ok".into())]).unwrap());
        stream.extend(error(4, "boom").unwrap());
        stream.extend(hello(&[Value::I32(1)]).unwrap());
        stream.extend(bye().unwrap());
        let mut r = &stream[..];
        assert_eq!(
            read(&mut r).unwrap(),
            Some(Frame::Call {
                id: 9,
                method: "blockState".into(),
                args: all_values()
            })
        );
        assert_eq!(
            read(&mut r).unwrap(),
            Some(Frame::Event {
                method: "log".into(),
                args: vec!["hi".into()]
            })
        );
        assert_eq!(
            read(&mut r).unwrap(),
            Some(Frame::Reply {
                id: 9,
                values: vec!["ok".into()]
            })
        );
        assert_eq!(
            read(&mut r).unwrap(),
            Some(Frame::Error {
                id: 4,
                message: "boom".into()
            })
        );
        assert_eq!(
            read(&mut r).unwrap(),
            Some(Frame::Hello {
                values: vec![Value::I32(1)]
            })
        );
        assert_eq!(read(&mut r).unwrap(), Some(Frame::Bye));
        assert_eq!(read(&mut r).unwrap(), None);
    }

    #[test]
    fn the_bytes_are_the_documented_layout() {
        // An independent expectation, written out by hand from the module docs: the Java side
        // is held to the same bytes by tests/java/WireCheck.java.
        let bytes = call(0x0102_0304, "ab", &[Value::I32(5), Value::Str("é".into())]).unwrap();
        let expected: Vec<u8> = vec![
            25, 0, 0, 0, // len: 1 + 4 + (4+2) + 2 + (1+4) + (1+4+2)
            1, // CALL
            4, 3, 2, 1, // id
            2, 0, 0, 0, b'a', b'b', // method
            2, 0, // two values
            2, 5, 0, 0, 0, // I32 5
            6, 2, 0, 0, 0, 0xC3, 0xA9, // Str "é"
        ];
        assert_eq!(bytes, expected);
    }

    #[test]
    fn damaged_frames_fail_instead_of_allocating_or_panicking() {
        // A length over the limit.
        let huge = [0xff, 0xff, 0xff, 0x7f, 1];
        assert!(read(&mut &huge[..]).is_err());
        // A string longer than its frame.
        let mut f = call(1, "x", &[Value::Str("abc".into())]).unwrap();
        let n = f.len();
        f[n - 7] = 200; // the string's length
        assert!(read(&mut &f[..]).is_err());
        // A string array claiming more entries than bytes.
        let mut f = call(1, "x", &[Value::Strs(vec![])]).unwrap();
        let n = f.len();
        f[n - 1] = 0x7f;
        assert!(read(&mut &f[..]).is_err());
        // Cut off mid-frame.
        let f = call(1, "x", &[Value::I64(1)]).unwrap();
        assert!(read(&mut &f[..f.len() - 2]).is_err());
        // An unknown kind.
        let mut f = bye().unwrap();
        f[4] = 77;
        assert!(read(&mut &f[..]).is_err());
    }
}
