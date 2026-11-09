package dev.horizonlab.remotetest;

import dev.horizonlab.bridge.remote.Host;
import dev.horizonlab.bridge.remote.Peer;
import dev.horizonlab.bridge.remote.RemoteRuntime;

/**
 * The child for horizon-jvm-remote's tests (tests/child.rs, tests/bench.rs): answers like a
 * runtime whose mods call back into Horizon. Not part of the bridge jar.
 */
public final class TestRuntime implements RemoteRuntime {
    private String[] args = new String[0];

    @Override
    public void init(Peer peer, String[] args) {
        this.args = args;
        Host.log("test runtime up with " + args.length + " args");
    }

    @Override
    public Object[] call(Peer peer, String method, Object[] a) throws Exception {
        switch (method) {
            case "ping":
                return new Object[] {"pong"};
            case "args":
                return new Object[] {args};
            case "echo":
                return a;
            case "nested": {
                // A tick whose mods read n block states from Horizon.
                int n = (Integer) a[0];
                long sum = 0;
                for (int i = 0; i < n; i++) {
                    Object[] r = peer.call("blockState", "minecraft:overworld", i, 64, -i);
                    sum += ((String) r[0]).length();
                }
                return new Object[] {sum};
            }
            case "deep": {
                // Calls nest both ways: Horizon's "down" calls "deep" again.
                int n = (Integer) a[0];
                if (n == 0) return new Object[] {"bottom"};
                Object[] r = peer.call("down", n);
                return new Object[] {n + "<" + r[0]};
            }
            case "events": {
                int n = (Integer) a[0];
                for (int i = 0; i < n; i++) peer.event("line", i);
                return new Object[] {n};
            }
            case "hang":
                // A mod stuck forever (Cobblemon 1.5's Showdown on Java 22+).
                Thread.sleep(Long.MAX_VALUE);
                return null;
            case "fail":
                throw new IllegalStateException("a mod threw");
            case "crash":
                System.out.println("crashing on purpose");
                System.out.flush();
                Runtime.getRuntime().halt(3);
                return null;
            default:
                throw new UnsupportedOperationException(method);
        }
    }
}
