package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.ServerFunctionManager;
import net.minecraft.world.level.timers.TimerQueue;

/**
 * The mods' data-pack functions: their {@code minecraft:load} and {@code minecraft:tick} tags
 * and what they {@code /schedule}, run on the mirrored server as vanilla's runs them
 * (MinecraftServer.tickChildren's commandFunctions, ServerLevel.tickTime's scheduled ones).
 * Mods that are data packs in a jar (Skyblock Infinite: its starting island is placed by its
 * load function) do their work here; their commands act on the mirrored levels, whose block
 * writes reach Horizon's. The world's own data packs are Horizon's (game/functions.rs): the
 * Java side's repository holds only vanilla's and the mods'.
 */
public final class HorizonFunctions {
    private HorizonFunctions() {}

    /** HORIZON_JAVA_FUNCTIONS_TRACE=1: every command the mods' functions run, and their errors, logged. */
    public static final boolean TRACE = "1".equals(System.getenv("HORIZON_JAVA_FUNCTIONS_TRACE"));

    public static net.minecraft.commands.execution.TraceCallbacks tracer() {
        return new net.minecraft.commands.execution.TraceCallbacks() {
            @Override public void onCommand(int depth, String command) { Natives.log("function> " + " ".repeat(depth) + command); }
            @Override public void onReturn(int depth, String command, int result) { }
            @Override public void onError(String message) { Natives.log("function! " + message); }
            @Override public void onCall(int depth, net.minecraft.resources.Identifier id, int size) { Natives.log("function> " + " ".repeat(depth) + "call " + id); }
            @Override public void close() { }
        };
    }

    private static ServerFunctionManager manager;
    private static TimerQueue<MinecraftServer> scheduled;
    private static boolean failed;

    /** Whether the mods' functions run now (this tick's), so a forced chunk not yet loaded can stop them. */
    private static boolean running;
    /** A function forced a chunk Horizon hasn't loaded: the rest of this run is dropped and the run retried. */
    private static boolean abort;
    private static final java.util.Set<String> WAITING = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /**
     * ServerLevel.setChunkForced (/forceload add) on a mirrored level: vanilla loads the chunk
     * there and then, and data packs rely on it (Skyblock Infinite forceloads 0 0, then places
     * its island there in the same function). Horizon loads it over the next ticks; until it
     * has, the function run that asked stops and runs again (FunctionsMixin), once a tick.
     */
    public static void forced(net.minecraft.server.level.ServerLevel level, int x, int z) {
        if (running && !level.hasChunk(x, z)) {
            if (WAITING.add(level.dimension().identifier() + " " + x + " " + z)) {
                Natives.log("Java mods: a function forced chunk " + x + " " + z + " of " + level.dimension().identifier()
                        + "; it runs again once Horizon has loaded it");
            }
            if (TRACE) Natives.log("function! waiting for chunk " + x + " " + z);
            abort = true;
        }
    }

    /** Whether the running commands must stop (FunctionsMixin drops the queue). */
    public static boolean aborting() {
        return running && abort;
    }

    /** Whether the run that just ended was stopped, and must run again; clears it. */
    public static boolean takeAborted() {
        boolean was = abort;
        abort = false;
        return was;
    }

    /** One server tick of functions, once a world is open (its saved data holds the schedule). */
    static void tick(MinecraftServer server, long gameTime) {
        if (failed || HorizonData.loaded == null || !HorizonSavedData.ready()) return;
        running = true;
        try {
            if (manager == null) start(server);
            manager.tick();
            // The load functions, stopped for a chunk: they run again next tick.
            if (takeAborted()) {
                java.lang.reflect.Field again = ServerFunctionManager.class.getDeclaredField("postReload");
                again.setAccessible(true);
                again.set(manager, true);
            }
            scheduled.tick(server, gameTime);
        } catch (Throwable t) {
            failed = true;
            Natives.log("Java mods: the mods' functions stopped: " + t);
            StackTraceElement[] frames = t.getStackTrace();
            for (int f = 0; f < Math.min(frames.length, 12); f++) Natives.log("    at " + frames[f]);
        } finally {
            running = false;
            abort = false;
        }
    }

    private static void start(MinecraftServer server) throws ReflectiveOperationException {
        // The world's spawn, which commands run from the server start at (the server's
        // command source): the world data's.
        java.lang.reflect.Field respawn = MinecraftServer.class.getDeclaredField("effectiveRespawnData");
        respawn.setAccessible(true);
        if (respawn.get(server) == null) respawn.set(server, server.getWorldData().overworldData().getRespawnData());
        var library = HorizonData.loaded.reloadable().getFunctionLibrary();
        // Its constructor reads the load and tick tags; the load ones run on the first tick.
        manager = new ServerFunctionManager(server, library);
        java.lang.reflect.Field field = MinecraftServer.class.getDeclaredField("functionManager");
        field.setAccessible(true);
        field.set(server, manager);
        scheduled = server.getDataStorage().computeIfAbsent(TimerQueue.TYPE);
        java.lang.reflect.Field events = MinecraftServer.class.getDeclaredField("scheduledEvents");
        events.setAccessible(true);
        events.set(server, scheduled);
        int names = 0;
        for (var ignored : manager.getFunctionNames()) names++;
        Natives.log("Java mods: " + names + " data-pack functions; " + manager.getTag(
                net.minecraft.resources.Identifier.withDefaultNamespace("load")).size() + " run at load, "
                + manager.getTag(net.minecraft.resources.Identifier.withDefaultNamespace("tick")).size() + " each tick");
    }
}
