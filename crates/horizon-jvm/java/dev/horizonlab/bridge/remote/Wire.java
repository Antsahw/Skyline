package dev.horizonlab.bridge.remote;

import java.io.DataInputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/**
 * The frames between Horizon and this JVM when it runs in a process of its own
 * (docs/java-bridge-120.md, "The transport"). Byte for byte the same as horizon-jvm-remote's
 * wire.rs, whose module docs describe the layout; little endian throughout.
 *
 * <p>Values are Java objects: {@code null}, Boolean, Integer, Long, Float, Double, String,
 * byte[], int[], String[] — the types Natives passes over JNI. JDK types only: this class runs
 * on the system class path, before and beside any Minecraft.
 */
public final class Wire {
    private Wire() {}

    public static final int PROTOCOL = 1;
    public static final int MAX_FRAME = 256 << 20;

    public static final byte CALL = 1, REPLY = 2, ERROR = 3, EVENT = 4, HELLO = 5, BYE = 6;

    /** A frame read off the connection. */
    public static final class Frame {
        public final byte kind;
        public final int id;
        /** The method of a CALL or EVENT, the message of an ERROR. */
        public final String text;
        public final Object[] values;

        Frame(byte kind, int id, String text, Object[] values) {
            this.kind = kind;
            this.id = id;
            this.text = text;
            this.values = values;
        }
    }

    /** Builds one frame. */
    public static final class Encoder {
        private ByteBuffer buf = ByteBuffer.allocate(256).order(ByteOrder.LITTLE_ENDIAN);

        public Encoder(byte kind, int id) {
            buf.putInt(0).put(kind).putInt(id);
        }

        private void room(int n) {
            if (buf.remaining() >= n) return;
            int size = Math.max(buf.capacity() * 2, buf.position() + n);
            ByteBuffer bigger = ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN);
            buf.flip();
            bigger.put(buf);
            buf = bigger;
        }

        public Encoder str(String s) {
            byte[] b = s.getBytes(StandardCharsets.UTF_8);
            room(4 + b.length);
            buf.putInt(b.length).put(b);
            return this;
        }

        public Encoder values(Object[] values) {
            if (values.length > 0xffff) throw new IllegalArgumentException("more than 65535 values");
            room(2);
            buf.putShort((short) values.length);
            for (Object v : values) value(v);
            return this;
        }

        private void value(Object v) {
            room(9);
            if (v == null) {
                buf.put((byte) 0);
            } else if (v instanceof Boolean b) {
                buf.put((byte) 1).put((byte) (b ? 1 : 0));
            } else if (v instanceof Integer i) {
                buf.put((byte) 2).putInt(i);
            } else if (v instanceof Long l) {
                buf.put((byte) 3).putLong(l);
            } else if (v instanceof Float f) {
                buf.put((byte) 4).putFloat(f);
            } else if (v instanceof Double d) {
                buf.put((byte) 5).putDouble(d);
            } else if (v instanceof String s) {
                buf.put((byte) 6);
                str(s);
            } else if (v instanceof byte[] b) {
                buf.put((byte) 7);
                room(4 + b.length);
                buf.putInt(b.length).put(b);
            } else if (v instanceof int[] a) {
                buf.put((byte) 8);
                room(4 + 4 * a.length);
                buf.putInt(a.length);
                for (int x : a) buf.putInt(x);
            } else if (v instanceof String[] a) {
                buf.put((byte) 9);
                room(4);
                buf.putInt(a.length);
                for (String s : a) str(s);
            } else {
                throw new IllegalArgumentException("no wire form for " + v.getClass().getName());
            }
        }

        /** The frame's bytes, length filled in. */
        public byte[] finish() {
            int len = buf.position() - 4;
            if (len > MAX_FRAME) throw new IllegalArgumentException("frame of " + len + " bytes is over the limit");
            buf.putInt(0, len);
            byte[] out = new byte[buf.position()];
            buf.flip();
            buf.get(out);
            return out;
        }
    }

    public static byte[] call(int id, String method, Object[] args) {
        return new Encoder(CALL, id).str(method).values(args).finish();
    }

    public static byte[] event(String method, Object[] args) {
        return new Encoder(EVENT, 0).str(method).values(args).finish();
    }

    public static byte[] reply(int id, Object[] values) {
        return new Encoder(REPLY, id).values(values).finish();
    }

    public static byte[] error(int id, String message) {
        return new Encoder(ERROR, id).str(message).finish();
    }

    public static byte[] hello(Object[] values) {
        return new Encoder(HELLO, 0).values(values).finish();
    }

    public static byte[] bye() {
        return new Encoder(BYE, 0).finish();
    }

    /** Reads one frame; {@code null} at a clean end of stream between frames. */
    public static Frame read(DataInputStream in) throws IOException {
        byte[] lenBytes = new byte[4];
        int got = in.readNBytes(lenBytes, 0, 4);
        if (got == 0) return null;
        if (got < 4) throw new EOFException("connection closed inside a frame");
        int len = ByteBuffer.wrap(lenBytes).order(ByteOrder.LITTLE_ENDIAN).getInt();
        if (len < 5 || len > MAX_FRAME) throw new IOException("frame length " + Integer.toUnsignedString(len));
        byte[] body = new byte[len];
        in.readFully(body);
        ByteBuffer b = ByteBuffer.wrap(body).order(ByteOrder.LITTLE_ENDIAN);
        byte kind = b.get();
        int id = b.getInt();
        Frame frame;
        try {
            switch (kind) {
                case CALL, EVENT -> frame = new Frame(kind, id, str(b), values(b));
                case REPLY, HELLO -> frame = new Frame(kind, id, null, values(b));
                case ERROR -> frame = new Frame(kind, id, str(b), new Object[0]);
                case BYE -> frame = new Frame(kind, id, null, new Object[0]);
                default -> throw new IOException("frame kind " + kind);
            }
        } catch (java.nio.BufferUnderflowException | IndexOutOfBoundsException e) {
            throw new IOException("frame ends inside a value");
        }
        if (b.hasRemaining()) throw new IOException(b.remaining() + " bytes left over in a frame");
        return frame;
    }

    private static int length(ByteBuffer b, int unit) throws IOException {
        int n = b.getInt();
        if (n < 0 || (long) n * unit > b.remaining()) throw new IOException("a value longer than its frame");
        return n;
    }

    private static String str(ByteBuffer b) throws IOException {
        int n = length(b, 1);
        String s = new String(b.array(), b.arrayOffset() + b.position(), n, StandardCharsets.UTF_8);
        b.position(b.position() + n);
        return s;
    }

    private static Object[] values(ByteBuffer b) throws IOException {
        int count = Short.toUnsignedInt(b.getShort());
        Object[] out = new Object[count];
        for (int i = 0; i < count; i++) out[i] = value(b);
        return out;
    }

    private static Object value(ByteBuffer b) throws IOException {
        byte tag = b.get();
        switch (tag) {
            case 0: return null;
            case 1: return b.get() != 0;
            case 2: return b.getInt();
            case 3: return b.getLong();
            case 4: return b.getFloat();
            case 5: return b.getDouble();
            case 6: return str(b);
            case 7: {
                byte[] out = new byte[length(b, 1)];
                b.get(out);
                return out;
            }
            case 8: {
                int[] out = new int[length(b, 4)];
                for (int i = 0; i < out.length; i++) out[i] = b.getInt();
                return out;
            }
            case 9: {
                String[] out = new String[length(b, 4)];
                for (int i = 0; i < out.length; i++) out[i] = str(b);
                return out;
            }
            default: throw new IOException("value tag " + tag);
        }
    }

    /** Wraps an input stream for {@link #read}. */
    public static DataInputStream input(InputStream in) {
        return new DataInputStream(new java.io.BufferedInputStream(in, 64 << 10));
    }
}
