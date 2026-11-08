package dev.horizonlab.bridge.remote;

import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * This JVM's end of the connection to Horizon: synchronous calls that nest both ways
 * (horizon-jvm-remote's peer.rs is the other end and explains why). The thread waiting for an
 * answer handles Horizon's calls and events meanwhile, so mod code running inside Horizon's
 * {@code tick} may call {@code blockState} back, and Horizon may call in again from there.
 *
 * <p>One connection is one conversation: reads and calls belong to the thread inside
 * {@link #serve} (the conversation thread). Any thread may send an event (a log line).
 */
public final class Peer {
    /** Answers Horizon's calls and takes its events. */
    public interface Handler {
        Object[] call(Peer peer, String method, Object[] args) throws Exception;

        default void event(Peer peer, String method, Object[] args) {}
    }

    /** Horizon's handler failed (its message), or the connection closed mid-call. */
    public static final class RemoteException extends RuntimeException {
        private static final long serialVersionUID = 1L;

        public RemoteException(String message) {
            super(message);
        }
    }

    private final DataInputStream in;
    private final OutputStream out;
    private final Object writeLock = new Object();
    private volatile Thread conversation;
    private Handler handler;
    private int nextId = 1;
    private boolean closed;

    public Peer(InputStream in, OutputStream out) {
        this.in = Wire.input(in);
        this.out = new BufferedOutputStream(out, 64 << 10);
    }

    private void send(byte[] frame, boolean flush) throws IOException {
        synchronized (writeLock) {
            out.write(frame);
            if (flush) out.flush();
        }
    }

    /** Sends the hello and returns Horizon's welcome (protocol, the runtime's arguments). */
    public Object[] hello(Object... values) throws IOException {
        send(Wire.hello(values), true);
        Wire.Frame f = Wire.read(in);
        if (f == null || f.kind != Wire.REPLY || f.id != 0) throw new IOException("Horizon didn't welcome this runtime");
        return f.values;
    }

    /**
     * Calls {@code method} in Horizon and waits for its answer. Only from the conversation
     * thread (inside {@link #serve}, or a handler it runs).
     */
    public Object[] call(String method, Object... args) {
        if (Thread.currentThread() != conversation) {
            throw new IllegalStateException("calls into Horizon go from the conversation thread, not " + Thread.currentThread().getName());
        }
        if (closed) throw new RemoteException("Horizon closed the connection");
        int id = nextId;
        nextId = nextId == Integer.MAX_VALUE ? 1 : nextId + 1;
        try {
            send(Wire.call(id, method, args), true);
            while (true) {
                Wire.Frame f = Wire.read(in);
                if (f == null || f.kind == Wire.BYE) {
                    closed = true;
                    throw new RemoteException("Horizon closed the connection");
                }
                if (f.kind == Wire.REPLY || f.kind == Wire.ERROR) {
                    if (f.id != id) throw new IOException("an answer to call " + f.id + " while waiting for " + id);
                    if (f.kind == Wire.ERROR) throw new RemoteException(f.text);
                    return f.values;
                }
                dispatch(f);
            }
        } catch (IOException e) {
            closed = true;
            throw new RemoteException("connection to Horizon: " + e);
        }
    }

    /**
     * Sends a one-way event; from any thread. From the conversation thread it goes out with
     * the next call or answer (they are always flushed); from any other thread at once.
     */
    public void event(String method, Object... args) {
        try {
            send(Wire.event(method, args), Thread.currentThread() != conversation);
        } catch (IOException e) {
            // Horizon is gone: the serve loop ends on its own.
        }
    }

    private void dispatch(Wire.Frame f) throws IOException {
        if (f.kind == Wire.CALL) {
            byte[] answer;
            try {
                Object[] values = handler.call(this, f.text, f.values);
                answer = Wire.reply(f.id, values == null ? new Object[0] : values);
            } catch (Throwable t) {
                Throwable root = t;
                if (root instanceof java.lang.reflect.InvocationTargetException && root.getCause() != null) root = root.getCause();
                answer = Wire.error(f.id, String.valueOf(root));
            }
            send(answer, true);
        } else if (f.kind == Wire.EVENT) {
            handler.event(this, f.text, f.values);
        } else {
            throw new IOException("unexpected frame kind " + f.kind);
        }
    }

    /** Handles Horizon's calls and events on this thread until it says goodbye. */
    public void serve(Handler handler) throws IOException {
        this.handler = handler;
        conversation = Thread.currentThread();
        while (!closed) {
            Wire.Frame f = Wire.read(in);
            if (f == null || f.kind == Wire.BYE) {
                closed = true;
                break;
            }
            dispatch(f);
        }
    }

    public boolean closed() {
        return closed;
    }
}
