//! A copy of the packets the server sends the window's own player (its memory connection:
//! single player, a LAN host), for the Java mods' client: recording mods (ReplayMod,
//! Flashback) read a client's whole packet stream from its network connection, and the
//! headless client's connection had none. The bodies (packet id and data, unframed) are kept
//! in order from the login's end, through configuration, into play, and taken each tick.
//!
//! Off unless Java asks: `undecided` while the Java mods start (a join's configuration can
//! come first), then `on` or `off` as they answered (`HorizonPacketTap.wanted`).

use std::sync::{
    Mutex, PoisonError,
    atomic::{AtomicU8, Ordering},
};

const OFF: u8 = 0;
const UNDECIDED: u8 = 1;
const ON: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(OFF);

/// What was kept while undecided stops growing past this (a big pack's registries and tags
/// are a few MiB; a world's first chunks come after Java answered, as joins wait for it).
const UNDECIDED_LIMIT: usize = 64 << 20;

#[derive(Default)]
struct Kept {
    uuid: Option<u128>,
    bodies: Vec<Vec<u8>>,
    bytes: usize,
}

static KEPT: Mutex<Kept> = Mutex::new(Kept {
    uuid: None,
    bodies: Vec::new(),
    bytes: 0,
});

fn kept() -> std::sync::MutexGuard<'static, Kept> {
    KEPT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Keep packets until Java answers whether it wants them.
pub fn undecided() {
    let _ = STATE.compare_exchange(OFF, UNDECIDED, Ordering::SeqCst, Ordering::SeqCst);
}

/// Java's answer: on keeps every packet from now, off drops what was kept.
pub fn want(on: bool) {
    STATE.store(if on { ON } else { OFF }, Ordering::SeqCst);
    if !on {
        *kept() = Kept::default();
    }
}

/// Whether packets are kept (on, or undecided).
pub fn keeping() -> bool {
    STATE.load(Ordering::Relaxed) != OFF
}

/// A memory connection's login ends for `uuid`: what is kept from now is theirs (a new world
/// of the same window starts a new stream). An empty body marks the start, for Java to open
/// a new connection there.
pub fn begin(uuid: u128) {
    if keeping() {
        *kept() = Kept {
            uuid: Some(uuid),
            bodies: vec![Vec::new()],
            bytes: 0,
        };
    }
}

/// One packet sent to the memory connection's player.
pub fn keep(body: &[u8]) {
    let state = STATE.load(Ordering::Relaxed);
    if state == OFF {
        return;
    }
    let mut kept = kept();
    if kept.uuid.is_none() || (state == UNDECIDED && kept.bytes > UNDECIDED_LIMIT) {
        return;
    }
    kept.bytes += body.len();
    kept.bodies.push(body.to_vec());
}

/// The player kept for and what was sent them since the last take, in order.
pub fn take() -> Option<(u128, Vec<Vec<u8>>)> {
    if STATE.load(Ordering::Relaxed) != ON {
        return None;
    }
    let mut kept = kept();
    let uuid = kept.uuid?;
    if kept.bodies.is_empty() {
        return None;
    }
    kept.bytes = 0;
    Some((uuid, std::mem::take(&mut kept.bodies)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test: the state is the process's.
    #[test]
    fn keeps_in_order_only_once_asked_and_from_the_login() {
        want(false);
        begin(7);
        keep(b"before");
        assert!(take().is_none(), "off keeps nothing");
        undecided();
        begin(7);
        keep(b"a");
        assert!(take().is_none(), "undecided keeps, gives nothing yet");
        want(true);
        keep(b"b");
        assert_eq!(take(), Some((7, vec![Vec::new(), b"a".to_vec(), b"b".to_vec()])));
        assert!(take().is_none(), "taken once");
        begin(9);
        keep(b"c");
        assert_eq!(take(), Some((9, vec![Vec::new(), b"c".to_vec()])));
        want(false);
        keep(b"d");
        assert!(take().is_none());
    }
}
