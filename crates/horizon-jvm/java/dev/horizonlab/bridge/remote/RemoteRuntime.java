package dev.horizonlab.bridge.remote;

/**
 * What a child JVM runs once connected to Horizon ({@link Child}): Fabric 1.20.1
 * ({@code dev.horizonlab.bridge.r120.Boot120}) or a test runtime. Its {@link #call} answers
 * Horizon's calls on the conversation thread.
 */
public interface RemoteRuntime extends Peer.Handler {
    /** Called once, on the conversation thread, before Horizon's first call. */
    default void init(Peer peer, String[] args) throws Exception {}

    /** Called after Horizon said goodbye, before the process exits. */
    default void stop() {}
}
