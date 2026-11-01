package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Where the mirror's server tick spends its time (HORIZON_JAVA_PROFILE=1): each phase's
 * nanoseconds summed, logged every 200 ticks as milliseconds a tick, the slowest first. The
 * game's side times its calls into Java the same way (game/java_profile.rs). Off, a phase costs
 * one check of a static.
 */
public final class HorizonProfile {
    private HorizonProfile() {}

    static final boolean ON = System.getenv("HORIZON_JAVA_PROFILE") != null;
    private static final Map<String, long[]> PHASES = new LinkedHashMap<>();
    private static int ticks;

    /** A phase's start (0 when off). */
    public static long start() {
        return ON ? System.nanoTime() : 0L;
    }

    /** A phase ended: its time since {@code started} added to {@code phase}; the next one's start. */
    public static long add(String phase, long started) {
        if (!ON) return 0L;
        long now = System.nanoTime();
        synchronized (PHASES) {
            PHASES.computeIfAbsent(phase, p -> new long[2])[0] += now - started;
            PHASES.get(phase)[1]++;
        }
        return now;
    }

    /** Per mod entity type: its entities' tick time and how many entity ticks (HorizonEntities). */
    private static final Map<String, long[]> ENTITIES = new java.util.HashMap<>();
    /** Per mod entity type: its renderer's time in the snapshot (HorizonEntityRender). */
    private static final Map<String, long[]> RENDERS = new java.util.HashMap<>();
    /** Block reads that went to Horizon (Natives.blockState): count, time. */
    private static final long[] READS = new long[2];
    /** Block reads answered by the tick's section copies (HorizonServer.blockState). */
    /** Block reads served (counted on any thread; a LongAdder: no lock per read). */
    private static final java.util.concurrent.atomic.LongAdder CACHED = new java.util.concurrent.atomic.LongAdder();

    /** An entity of {@code type} ticked in {@code nanos}. */
    public static void entity(String type, long nanos) {
        if (!ON) return;
        synchronized (ENTITIES) {
            long[] t = ENTITIES.computeIfAbsent(type, k -> new long[2]);
            t[0] += nanos;
            t[1]++;
        }
    }

    /** An entity of {@code type} drawn for the snapshot in {@code nanos}. */
    public static void render(String type, long nanos) {
        if (!ON) return;
        synchronized (RENDERS) {
            long[] t = RENDERS.computeIfAbsent(type, k -> new long[2]);
            t[0] += nanos;
            t[1]++;
        }
    }

    /** A block read Horizon answered (a JNI call), in {@code nanos}. */
    public static void read(long nanos) {
        if (!ON) return;
        synchronized (READS) {
            READS[0]++;
            READS[1] += nanos;
        }
    }

    /** Sections kept from an earlier view, their chunk unchanged. */
    private static final long[] REUSED = new long[1];

    /** A section kept from an earlier view (its chunk unchanged) served this one. */
    public static void reused() {
        if (!ON) return;
        synchronized (REUSED) {
            REUSED[0]++;
        }
    }

    /** A block read the section copies answered. */
    public static void cached() {
        if (!ON) return;
        {
            CACHED.increment();
        }
    }

    private static String perType(String what, Map<String, long[]> map, int ticks) {
        StringBuilder line = new StringBuilder("Java profile (" + what + "):");
        synchronized (map) {
            long all = 0, n = 0;
            for (long[] t : map.values()) {
                all += t[0];
                n += t[1];
            }
            line.append(String.format(" %.2f ms a tick, %.0f a tick;", all / 1e6 / ticks, (double) n / ticks));
            map.entrySet().stream()
                    .sorted((a, b) -> Long.compare(b.getValue()[0], a.getValue()[0]))
                    .limit(12)
                    .forEach(e -> line.append(String.format(" %s %.2f ms (%.0f, %.3f each)", e.getKey(),
                            e.getValue()[0] / 1e6 / ticks, (double) e.getValue()[1] / ticks,
                            e.getValue()[0] / 1e6 / Math.max(1, e.getValue()[1]))));
            map.clear();
        }
        return line.toString();
    }

    /** The end of a server tick: every 200, the report. */
    static void tick() {
        if (!ON || ++ticks < 200) return;
        StringBuilder line = new StringBuilder("Java profile (mirror tick):");
        synchronized (PHASES) {
            double total = 0;
            // Parts of a phase ("worldgen terrain: surface") are inside it: not added again.
            for (var e : PHASES.entrySet()) if (!e.getKey().contains(": ")) total += e.getValue()[0];
            line.append(String.format(" %.2f ms a tick;", total / 1e6 / ticks));
            PHASES.entrySet().stream()
                    .sorted((a, b) -> Long.compare(b.getValue()[0], a.getValue()[0]))
                    .limit(14)
                    .forEach(e -> line.append(e.getKey().startsWith("worldgen")
                            // Worldgen runs a chunk at a time: its time a chunk too.
                            ? String.format(" %s %.2f ms (%.1f ms each)", e.getKey(), e.getValue()[0] / 1e6 / ticks,
                                    e.getValue()[0] / 1e6 / Math.max(1, e.getValue()[1]))
                            : String.format(" %s %.2f ms", e.getKey(), e.getValue()[0] / 1e6 / ticks)));
            PHASES.clear();
        }
        Natives.log(line.toString());
        Natives.log(perType("entity ticks", ENTITIES, ticks));
        Natives.log(perType("entity snapshot", RENDERS, ticks));
        synchronized (READS) {
            Natives.log(String.format("Java profile (block reads): %.0f section fetches a tick from Horizon, %.2f ms (%.2f us each); %.0f kept from earlier ticks; %.0f block reads a tick from them",
                    (double) READS[0] / ticks, READS[1] / 1e6 / ticks, READS[1] / 1e3 / Math.max(1, READS[0]),
                    (double) REUSED[0] / ticks, (double) CACHED.sum() / ticks));
            REUSED[0] = 0;
            READS[0] = READS[1] = 0;
            CACHED.reset();
        }
        ticks = 0;
    }

    /**
     * HORIZON_JAVA_DEBUG=stall: every 30 s, every Java thread's stack goes to
     * horizon-java/stacks.txt in the game folder (the JVM runs with -Xrs: no jstack, no
     * SIGQUIT dump), to see where a hung start or tick is.
     */
    public static void stallWatch() {
        if (!String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("stall") || watching) return;
        watching = true;
        Thread t = new Thread(() -> {
            while (true) {
                try {
                    Thread.sleep(30_000);
                    StringBuilder out = new StringBuilder();
                    for (var e : Thread.getAllStackTraces().entrySet()) {
                        out.append('"').append(e.getKey().getName()).append("\" ").append(e.getKey().getState()).append('\n');
                        for (StackTraceElement f : e.getValue()) out.append("    at ").append(f).append('\n');
                        out.append('\n');
                    }
                    var file = net.fabricmc.loader.api.FabricLoader.getInstance().getGameDir().resolve("horizon-java/stacks.txt");
                    java.nio.file.Files.createDirectories(file.getParent());
                    java.nio.file.Files.writeString(file, out);
                } catch (Throwable ignored) {
                    return;
                }
            }
        }, "Horizon stall watch");
        t.setDaemon(true);
        t.start();
    }

    private static volatile boolean watching;
}
