package dev.horizonlab.bridge.remote;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.StandardProtocolFamily;
import java.net.UnixDomainSocketAddress;
import java.nio.channels.Channels;
import java.nio.channels.SocketChannel;
import java.nio.file.Path;

/**
 * The entry point of a Java runtime in a process of its own (docs/java-bridge-120.md):
 *
 * <pre>java -cp ... dev.horizonlab.bridge.remote.Child --connect unix:PATH|tcp:HOST:PORT --runtime CLASS</pre>
 *
 * Connects to the socket Horizon listens on, sends the hello with the token from
 * {@code HORIZON_REMOTE_TOKEN}, takes the runtime's arguments from Horizon's welcome (UTF-8 on
 * the wire: the command line would be decoded in the system's locale), makes the runtime and serves Horizon's calls on this thread
 * (the conversation thread) until Horizon says goodbye or goes away, then exits. Started by
 * horizon-jvm-remote's child.rs. Java 17+, JDK types only.
 */
public final class Child {
    private Child() {}

    public static void main(String[] args) throws Exception {
        String connect = null, runtimeClass = null;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--connect") && i + 1 < args.length) connect = args[++i];
            else if (args[i].equals("--runtime") && i + 1 < args.length) runtimeClass = args[++i];
        }
        if (connect == null || runtimeClass == null) {
            System.err.println("usage: Child --connect unix:PATH|tcp:HOST:PORT --runtime CLASS");
            System.exit(2);
        }
        String token = System.getenv("HORIZON_REMOTE_TOKEN");
        if (token == null) token = "";
        SocketChannel channel = open(connect);
        InputStream in = Channels.newInputStream(channel);
        OutputStream out = Channels.newOutputStream(channel);
        Peer peer = new Peer(in, out);
        Object[] welcome = peer.hello(Wire.PROTOCOL, token, runtimeClass, Runtime.version().toString(), ProcessHandle.current().pid());
        String[] runtimeArgs = welcome.length > 1 && welcome[1] instanceof String[] a ? a : new String[0];
        Host.attach(peer);
        int code = 0;
        RemoteRuntime runtime = null;
        try {
            runtime = (RemoteRuntime) Class.forName(runtimeClass).getConstructor().newInstance();
            runtime.init(peer, runtimeArgs);
            peer.serve(runtime);
        } catch (Throwable t) {
            System.err.println("[horizon-remote] the runtime failed: " + t);
            t.printStackTrace();
            code = 1;
        } finally {
            if (runtime != null) {
                try {
                    runtime.stop();
                } catch (Throwable t) {
                    t.printStackTrace();
                }
            }
        }
        // Mods leave non-daemon threads behind: the process ends with the conversation.
        System.exit(code);
    }

    private static SocketChannel open(String address) throws IOException {
        if (address.startsWith("unix:")) {
            SocketChannel channel = SocketChannel.open(StandardProtocolFamily.UNIX);
            channel.connect(UnixDomainSocketAddress.of(Path.of(address.substring(5))));
            return channel;
        }
        if (address.startsWith("tcp:")) {
            String hostPort = address.substring(4);
            int colon = hostPort.lastIndexOf(':');
            SocketChannel channel = SocketChannel.open();
            channel.connect(new InetSocketAddress(hostPort.substring(0, colon), Integer.parseInt(hostPort.substring(colon + 1))));
            // Small frames go out at once: Nagle's delay would add ~40 ms to a call.
            channel.socket().setTcpNoDelay(true);
            return channel;
        }
        throw new IllegalArgumentException("unknown address " + address);
    }
}
