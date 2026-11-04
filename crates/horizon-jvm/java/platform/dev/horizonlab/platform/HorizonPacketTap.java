package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.embedded.EmbeddedChannel;
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.client.multiplayer.ClientConfigurationPacketListenerImpl;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.network.Connection;
import net.minecraft.network.PacketListener;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.BundlePacket;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.configuration.ClientConfigurationPacketListener;
import net.minecraft.network.protocol.configuration.ClientboundFinishConfigurationPacket;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.login.ClientLoginPacketListener;
import net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket;
import net.minecraft.network.protocol.login.LoginProtocols;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayDeque;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * The window's own player's client connection, with the packets its server sends it.
 *
 * <p>A vanilla client reads its server through a {@code Connection} on a netty channel:
 * splitter, decoder (the protocol of the phase), the connection itself as the packet handler.
 * Recording mods sit in that path: ReplayMod adds handlers before and after the decoder at the
 * login's end (the raw bytes and the decoded packets), Flashback injects into
 * {@code Connection.genericsFtw} (every packet handed to its listener). The headless client's
 * connection had no channel, and Horizon's server sends the player's game, not Java, so they
 * saw nothing.
 *
 * <p>When a mod hooks the connection's read path ({@link #wanted}), Horizon keeps the bodies it
 * sends the window's own player (single player, a LAN host: its memory connection) from the
 * login's end and passes them each tick ({@code clientPackets}). Here they go, in order, into a
 * real vanilla pipeline on an {@link EmbeddedChannel} ({@code Connection.configureSerialization},
 * {@code configurePacketHandler}) past its splitter (they come one packet each, and a memory
 * connection's are not held to 2 MiB), so the mods' handlers see them as on a client. Its
 * phases switch where vanilla's listeners switch them (after the login's end and
 * configuration's end, the terminal packets).
 *
 * <p>Nothing is handled as vanilla's client would (Horizon's game is the player's client):
 * the connection hands each packet to a listener of the phase's interface whose handlers run
 * only the mods' injections at the head and end of the real listener's handler (the login's
 * and configuration's listeners made as the client's, then the player's own ClientPacketListener),
 * not vanilla's body. The join's login and spawn are fired by {@code HorizonClient.joined}
 * already, and mods' payloads reach their client code through HorizonLoop: those stay out.
 * The connection never sends (the player's game does).
 */
public final class HorizonPacketTap {
    private HorizonPacketTap() {}

    private static volatile Boolean wanted;
    /** The mods that read the connection, by name (the player's line). */
    private static String readers = "";
    /** The player whose game is this window (the tap's), as Rust named them. */
    private static volatile UUID owner;
    /** Bodies passed and not run through the pipeline yet (waiting for the owner's client). */
    private static final ArrayDeque<byte[]> WAITING = new ArrayDeque<>();
    private static Connection connection;
    private static EmbeddedChannel channel;
    private static ClientPacketListener play;
    private static final Set<Connection> TAPPED = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<>());
    private static final Map<String, Integer> FAILURES = new ConcurrentHashMap<>();
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("tap");
    /** Handlers whose mods' injections run elsewhere: the join's (HorizonClient.joined), payloads (HorizonLoop). */
    private static final Set<String> FIRED_ELSEWHERE = Set.of("handleLogin", "handleSetSpawn", "handleCustomPayload",
            "handleCustomQuery", "handleCookieRequest");
    private static int packets;
    private static boolean told;

    /**
     * Whether a mod reads a client connection's packets: a mod's mixin merged into
     * {@code Connection} or the login's listener (where a client's connection is handed over
     * at the login's end), other than Fabric's and Horizon's own. Only in a headless client
     * with its server in this process (the window's own player's game).
     */
    public static boolean wanted() {
        if (wanted != null) return wanted;
        boolean found = false;
        var mods = new java.util.TreeSet<String>();
        if (HorizonClient.headless()) {
            for (Class<?> owner : new Class<?>[] {Connection.class, ClientHandshakePacketListenerImpl.class}) {
                for (Method m : owner.getDeclaredMethods()) {
                    var merged = m.getAnnotation(org.spongepowered.asm.mixin.transformer.meta.MixinMerged.class);
                    if (merged == null || merged.mixin().startsWith("dev.horizonlab.") || merged.mixin().startsWith("net.fabricmc.")) continue;
                    Natives.log("Horizon's client: " + merged.mixin() + " reads the client's connection: its packets are passed");
                    String mod = HorizonFailures.modOf(merged.mixin());
                    if (mod != null) mods.add(HorizonFailures.modName(mod));
                    found = true;
                    break;
                }
            }
        }
        wanted = found;
        readers = String.join(", ", mods);
        return found;
    }

    /** Whether this is the tapped connection (its sends dropped, its faults only logged). */
    public static boolean tapped(Object connection) {
        synchronized (TAPPED) {
            return TAPPED.contains(connection);
        }
    }

    /** What the server sent the window's player, framed by Rust: an int length before each body. */
    static void feed(String uuid, byte[] framed) {
        UUID id = UUID.fromString(uuid);
        synchronized (WAITING) {
            if (!id.equals(owner)) {
                WAITING.clear();
                owner = id;
            }
            ByteBuf in = Unpooled.wrappedBuffer(framed);
            while (in.readableBytes() >= 4) {
                byte[] body = new byte[in.readInt()];
                in.readBytes(body);
                WAITING.add(body);
            }
        }
    }

    /**
     * Inside the player's client (HorizonClient's tick, as them, after their join): the bodies
     * waiting go through their connection's pipeline. Only the owner's.
     */
    static void pump(net.minecraft.server.level.ServerPlayer player) {
        if (!player.getUUID().equals(owner)) return;
        while (true) {
            byte[] body;
            synchronized (WAITING) {
                body = WAITING.poll();
            }
            if (body == null) return;
            try {
                // A login's end (Rust's mark, an empty body): a new connection; the last one, of
                // another world of this window, ends first.
                if (body.length == 0) {
                    open(player);
                    continue;
                }
                if (channel == null) continue;
                // Past the splitter: one packet each.
                channel.pipeline().context("splitter").fireChannelRead(Unpooled.wrappedBuffer(body));
                channel.runPendingTasks();
                packets++;
            } catch (Throwable t) {
                fault("a packet couldn't be read", t);
            }
            // What the mods' handlers wrote outbound (none goes anywhere: the player's game sends).
            if (channel != null) channel.releaseOutbound();
        }
    }

    /** The player's connection, as a client's is at its login's end: a pipeline, the login's listener. */
    private static void open(net.minecraft.server.level.ServerPlayer player) throws Throwable {
        close(false);
        var minecraft = HorizonClient.minecraft();
        Connection c = new Connection(PacketFlow.CLIENTBOUND);
        HorizonServer.mirror(c);
        synchronized (TAPPED) {
            TAPPED.add(c);
        }
        EmbeddedChannel ch = new EmbeddedChannel(new ChannelInitializer<>() {
            @Override
            protected void initChannel(io.netty.channel.Channel channel) {
                // Connection.connect's client pipeline (no timeout: nothing reads the channel).
                Connection.configureSerialization(channel.pipeline(), PacketFlow.CLIENTBOUND, false, null);
                c.configurePacketHandler(channel.pipeline());
            }
        });
        connection = c;
        channel = ch;
        packets = 0;
        // The login's listener (made as the client's, without its constructor's work).
        var login = (ClientHandshakePacketListenerImpl) Natives.allocate(ClientHandshakePacketListenerImpl.class);
        HorizonClient.set(ClientHandshakePacketListenerImpl.class, login, "connection", c);
        HorizonClient.set(ClientHandshakePacketListenerImpl.class, login, "minecraft", minecraft);
        HorizonClient.set(ClientHandshakePacketListenerImpl.class, login, "seenPlayers", new java.util.HashMap<>());
        HorizonClient.set(ClientHandshakePacketListenerImpl.class, login, "cookies", new java.util.HashMap<>());
        c.setupInboundProtocol(LoginProtocols.CLIENTBOUND, login);
        // The player's own client connection reads through it (getConnection().getConnection()).
        play = HorizonClient.connection(player);
        if (play != null) HorizonClient.set(ClientCommonPacketListenerImpl.class, play, "connection", c);
        Natives.log("Horizon's client: " + player.getName().getString() + "'s client connection reads its packets");
        // What they get here, once a session: recordings, and where to watch them.
        if (!told) {
            told = true;
            HorizonFailures.note(player.getStringUUID(), "Your client's packets go to " + (readers.isEmpty() ? "a mod" : readers)
                    + " (recordings save as the world closes). Horizon plays ReplayMod's .mcpr recordings: "
                    + "start it with --replay <file> (the launcher's game arguments)");
        }
    }

    /**
     * The listener the connection hands a packet to: the phase's interface, whose handlers run
     * the mods' injections into {@code real}'s (Connection.genericsFtw's argument, ConnectionMixin).
     */
    public static PacketListener listener(PacketListener real) {
        Class<?> phase = real instanceof ClientGamePacketListener ? ClientGamePacketListener.class
                : real instanceof ClientConfigurationPacketListener ? ClientConfigurationPacketListener.class
                : real instanceof ClientLoginPacketListener ? ClientLoginPacketListener.class : null;
        if (phase == null) return real;
        return LISTENERS.computeIfAbsent(real, r -> (PacketListener) Proxy.newProxyInstance(phase.getClassLoader(),
                new Class<?>[] {phase}, handler(r)));
    }

    private static final Map<PacketListener, PacketListener> LISTENERS =
            java.util.Collections.synchronizedMap(new java.util.WeakHashMap<>());

    private static InvocationHandler handler(PacketListener real) {
        return (proxy, method, args) -> {
            if (method.getDeclaringClass() == Object.class) {
                return switch (method.getName()) {
                    case "equals" -> proxy == args[0];
                    case "hashCode" -> System.identityHashCode(proxy);
                    default -> "HorizonPacketTap listener of " + real;
                };
            }
            if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, args);
            if (method.getName().startsWith("handle") && args != null && args.length == 1) {
                handle((PacketListener) proxy, real, method.getName(), method.getParameterTypes()[0], args[0]);
                return null;
            }
            Class<?> type = method.getReturnType();
            if (type == boolean.class) return true; // isAcceptingMessages
            if (type == void.class || !type.isPrimitive()) {
                if (method.getName().equals("protocol") || method.getName().equals("flow")) return method.invoke(real, args);
                return null;
            }
            return 0;
        };
    }

    private static void handle(PacketListener proxy, PacketListener real, String name, Class<?> type, Object packet) {
        if (DEBUG) Natives.log("tap: " + name + " " + packet.getClass().getSimpleName());
        if (!FIRED_ELSEWHERE.contains(name)) fire(real, name, type, packet, "HEAD");
        try {
            // A bundle's packets, each to the listener as ClientPacketListener.handleBundlePacket does.
            if (packet instanceof BundlePacket<?> bundle) {
                for (var sub : bundle.subPackets()) {
                    @SuppressWarnings({"unchecked", "rawtypes"})
                    Packet<PacketListener> p = (Packet) sub;
                    p.handle(proxy);
                }
            }
            // The phases switch where vanilla's listeners switch them.
            if (packet instanceof ClientboundLoginFinishedPacket) {
                var config = (ClientConfigurationPacketListenerImpl) Natives.allocate(ClientConfigurationPacketListenerImpl.class);
                HorizonClient.set(ClientCommonPacketListenerImpl.class, config, "connection", connection);
                HorizonClient.set(ClientCommonPacketListenerImpl.class, config, "minecraft", HorizonClient.minecraft());
                unlisten();
                connection.setupInboundProtocol(ConfigurationProtocols.CLIENTBOUND, config);
            } else if (packet instanceof ClientboundFinishConfigurationPacket && play != null) {
                var registries = HorizonServer.server().registryAccess();
                // The mods' injections at the handler's own call (ReplayMod records the
                // configuration's end there, with the registries).
                HorizonHooks.fireMergedSkipping(real, ClientConfigurationPacketListenerImpl.class, name,
                        "INVOKE Lnet/minecraft/network/Connection;setupInboundProtocol", true, packet, registries);
                unlisten();
                connection.setupInboundProtocol(GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(registries)), play);
            }
        } catch (Throwable t) {
            fault("the connection's " + name, t);
        }
        if (!FIRED_ELSEWHERE.contains(name)) {
            fire(real, name, type, packet, "RETURN"); // TAIL and RETURN as one
        }
    }

    /**
     * The connection lets go of its listener before the next phase's (and before it ends):
     * Fabric's networking ends the old listener's session there (ConnectionMixin), and the
     * listeners here, made without their constructors, have no Fabric session.
     */
    private static void unlisten() throws ReflectiveOperationException {
        HorizonClient.set(Connection.class, connection, "packetListener", null);
    }

    /** The mods' injections at {@code at} of the handler, wherever the real listener's classes declare it. */
    private static void fire(PacketListener real, String name, Class<?> type, Object packet, String at) {
        for (Class<?> c = real.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            try {
                c.getDeclaredMethod(name, type);
            } catch (NoSuchMethodException e) {
                continue;
            }
            // Read from the merged class (mods that apply their mixin configs themselves:
            // ReplayMod's are its launcher's), Fabric's own left out (its networking's
            // session, which these listeners lack).
            HorizonHooks.fireMergedSkipping(real, c, name, at, true, packet);
        }
    }

    /** The tapped connection fails (a packet it can't decode): logged, never disconnected. */
    public static void fault(String where, Throwable t) {
        Throwable cause = t;
        while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
        if (FAILURES.merge(where + cause.getClass().getName(), 1, Integer::sum) <= 3) {
            Natives.log("Horizon's client: " + where + " (the client's connection goes on): " + cause);
        }
    }

    /** The owner left (or the world closes): their connection ends. */
    static void left(UUID id) {
        if (id.equals(owner)) close(true);
    }

    /**
     * The connection ends as a client's does when its server goes (Connection.disconnect:
     * channelInactive down the pipeline, where ReplayMod saves its recording on threads of its
     * own); with {@code wait}, what those start is waited for (up to 30 s), as a quitting game
     * would leave them unfinished.
     */
    static void close(boolean wait) {
        if (channel == null) return;
        var before = Thread.getAllStackTraces().keySet();
        try {
            unlisten();
            connection.disconnect(Component.translatable("multiplayer.disconnect.server_shutdown"));
            channel.close();
            channel.runPendingTasks();
        } catch (Throwable t) {
            fault("the connection's end", t);
        }
        // Fabric's DISCONNECT, as the client's connection ends (Flashback finishes its recording).
        var listener = play;
        if (listener != null) {
            HorizonServer.fireEach("ClientPlayConnectionEvents.DISCONNECT",
                    net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.DISCONNECT,
                    l -> l.onPlayDisconnect(listener, HorizonClient.minecraft()));
        }
        play = null;
        channel = null;
        Natives.log("Horizon's client: the client's connection ended after " + packets + " packets");
        if (!wait) return;
        // Run what the mods queued on the client (ReplayMod's runLater) while their threads work.
        long until = System.currentTimeMillis() + 30_000;
        for (Thread t : Thread.getAllStackTraces().keySet()) {
            if (before.contains(t) || t == Thread.currentThread() || t.isDaemon()) continue;
            while (t.isAlive() && System.currentTimeMillis() < until) {
                HorizonClient.tasks();
                try {
                    t.join(50);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    return;
                }
            }
        }
        HorizonClient.tasks();
    }
}
