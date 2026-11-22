package dev.horizonlab.bridge.remote;

/**
 * Horizon, seen from a child JVM: what {@code Natives} is in the bridge's own JVM, over the
 * socket instead of JNI. Calls go from the conversation thread; {@link #log} from any.
 * Outside a child (a standalone run) it prints instead.
 */
public final class Host {
    private Host() {}

    private static volatile Peer peer;

    static void attach(Peer p) {
        peer = p;
    }

    public static boolean attached() {
        return peer != null;
    }

    /** A line for Horizon's log. */
    public static void log(String line) {
        Peer p = peer;
        if (p != null) p.event("log", line);
        else System.out.println("[horizon-remote] " + line);
    }

    /** An event for Horizon (see {@link Peer#event}): from the conversation thread, sent with the reply. */
    public static void event(String method, Object... args) {
        Peer p = peer;
        if (p == null) throw new IllegalStateException("not running under Horizon");
        p.event(method, args);
    }

    /** A call into Horizon (see {@link Peer#call}). */
    public static Object[] call(String method, Object... args) {
        Peer p = peer;
        if (p == null) throw new IllegalStateException("not running under Horizon");
        return p.call(method, args);
    }

    /** Runs {@code work} on a thread of {@code group} as the conversation's (see {@link Peer#lend}). */
    public static void lend(ThreadGroup group, String name, Runnable work) {
        Peer p = peer;
        if (p != null) {
            p.lend(group, name, work);
            return;
        }
        Throwable[] failed = new Throwable[1];
        Thread worker = new Thread(group, () -> {
            try {
                work.run();
            } catch (Throwable t) {
                failed[0] = t;
            }
        }, name);
        worker.start();
        try {
            worker.join();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        if (failed[0] instanceof RuntimeException r) throw r;
        if (failed[0] instanceof Error e) throw e;
    }
}
