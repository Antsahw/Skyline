package dev.horizonlab.platform120;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;
import net.fabricmc.fabric.api.event.Event;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerWorldEvents;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;

/**
 * Fabric API's server events, fired for the 1.20.1 mods as a real server fires them (phase 5):
 * the server starting and started, each level loading, and the start and end of every server
 * tick and level tick, paced by Horizon's.
 *
 * <p>Each mod's listener is called on its own, not through the event's invoker: a mod whose
 * listener throws here (it reaches for something the mirror hasn't got) is said once and
 * skipped, and the next mod's listener still runs. Through the invoker, the first failure
 * would silence every listener after it.
 */
final class Events120 {
    private Events120() {}

    private static final Set<String> FAILED = ConcurrentHashMap.newKeySet();
    private static boolean started;
    private static long lastServerTick = Long.MIN_VALUE;

    /** The listeners an event holds (Fabric's ArrayBackedEvent keeps them in `handlers`). */
    static Object[] handlers(Event<?> event) {
        for (Class<?> c = event.getClass(); c != null; c = c.getSuperclass()) {
            try {
                var field = c.getDeclaredField("handlers");
                field.setAccessible(true);
                return (Object[]) field.get(event);
            } catch (NoSuchFieldException e) {
                // Up the hierarchy.
            } catch (ReflectiveOperationException | RuntimeException e) {
                return null;
            }
        }
        return null;
    }

    /** How many listeners an event has; -1 when it can't be told (then it is fired anyway). */
    static int listeners(Event<?> event) {
        Object[] h = handlers(event);
        return h == null ? -1 : h.length;
    }

    /** Calls each of `event`'s listeners with `call`, each on its own. */
    @SuppressWarnings("unchecked")
    static <T> void fire(Event<T> event, String what, Consumer<T> call) {
        Object[] h = handlers(event);
        if (h == null) {
            run(what, event.invoker(), call);
            return;
        }
        for (Object listener : h) run(what, (T) listener, call);
    }

    private static <T> void run(String what, T listener, Consumer<T> call) {
        try {
            call.accept(listener);
        } catch (RuntimeException | LinkageError e) {
            String who = listener.getClass().getName();
            if (FAILED.add(what + " " + who)) {
                Mirror120.log(what + ": " + who + " failed and is skipped from now on in this place: " + e);
            }
        }
    }

    /** The server starting and started, and the levels made so far loading: once. */
    static synchronized void start(MinecraftServer server, Iterable<ServerLevel> levels) {
        if (started) return;
        started = true;
        fire(ServerLifecycleEvents.SERVER_STARTING, "server starting", l -> l.onServerStarting(server));
        for (ServerLevel level : levels) loaded(server, level);
        fire(ServerLifecycleEvents.SERVER_STARTED, "server started", l -> l.onServerStarted(server));
    }

    /** A level made after the server started loads, as a server's does when it comes up. */
    static void loaded(MinecraftServer server, ServerLevel level) {
        fire(ServerWorldEvents.LOAD, "level load", l -> l.onWorldLoad(server, level));
    }

    /**
     * A player joins, as the 1.20.1 mods know players: when Horizon's player first reaches the
     * mods' code (their first click, break, screen or tick with them in it). Fabric API's
     * ServerPlayConnectionEvents INIT then JOIN, with a sender that reaches the player's client
     * half in this runtime (Packets120): what a mod sends a player who joins (its config, its
     * synced data) arrives.
     */
    static void joined(MinecraftServer server, net.minecraft.server.level.ServerPlayer player) {
        var connection = player.connection;
        var sender = Packets120.serverSender(player);
        fire(net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents.INIT, "player init",
                l -> l.onPlayInit(connection, server));
        fire(net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents.JOIN, "player join",
                l -> l.onPlayReady(connection, sender, server));
    }

    /** A player leaves: Fabric API's ServerPlayConnectionEvents.DISCONNECT. */
    static void left(MinecraftServer server, net.minecraft.server.level.ServerPlayer player) {
        var connection = player.connection;
        fire(net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents.DISCONNECT, "player leave",
                l -> l.onPlayDisconnect(connection, server));
    }

    static boolean started() {
        return started;
    }

    /** Whether any mod listens to a tick, server or client: a tick nobody hears costs nothing. */
    static boolean anyTickListeners() {
        return listeners(ServerTickEvents.START_SERVER_TICK) != 0
                || listeners(ServerTickEvents.END_SERVER_TICK) != 0
                || listeners(ServerTickEvents.START_WORLD_TICK) != 0
                || listeners(ServerTickEvents.END_WORLD_TICK) != 0
                || listeners(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.START_CLIENT_TICK) != 0
                || listeners(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK) != 0;
    }

    /**
     * One tick of `level` at server tick `tick`: the server's tick events once per server tick
     * (the first level ticked in it), its level's around them, as MinecraftServer.tickServer
     * then tickChildren run them.
     */
    static void tick(MinecraftServer server, ServerLevel level, long tick) {
        boolean newServerTick = tick != lastServerTick;
        lastServerTick = tick;
        if (newServerTick) {
            fire(ServerTickEvents.START_SERVER_TICK, "server tick start", l -> l.onStartTick(server));
        }
        fire(ServerTickEvents.START_WORLD_TICK, "level tick start", l -> l.onStartTick(level));
        fire(ServerTickEvents.END_WORLD_TICK, "level tick end", l -> l.onEndTick(level));
        if (newServerTick) {
            fire(ServerTickEvents.END_SERVER_TICK, "server tick end", l -> l.onEndTick(server));
        }
    }
}
