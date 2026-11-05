package dev.horizonlab.bridge.c112;

import java.util.concurrent.SynchronousQueue;
import java.util.concurrent.TimeUnit;
import java.util.function.Function;

import dev.horizonlab.bridge.Natives;

/**
 * Starts Forge 1.12.2 (Cleanroom) inside Horizon's JVM, hidden (docs/java-bridge-112.md), the
 * counterpart of FabricBoot and NeoBoot. On the system class path beside Cleanroom's libraries
 * and Mojang's 1.12.2 client jar (horizon-jvm's boot112.rs).
 *
 * Cleanroom's launcher ({@code top.outlands.foundation.boot.Foundation} with FML's client
 * tweaker) runs on a thread of its own, which becomes 1.12's client thread: it makes the hidden
 * window and its GL context (SDL's offscreen driver), loads the mods (preInit, init, postInit,
 * texture stitching, model baking) and opens the main menu. On the first client tick the
 * platform mod ({@code horizon112}, java/platform112/mod) hands over a report and a command
 * handler, then keeps that thread for Horizon: it runs commands and never returns to 1.12's
 * own game loop.
 *
 * The hand-over goes through {@link System#getProperties()}: the platform mod lives in
 * Foundation's class loader, which would load its own copy of this class from the class path,
 * so only JDK types cross (a queue, a {@code Function<String, String>}).
 *
 * Nothing here comes from Cleanroom's source (CleanroomMC License 1.0, source-available):
 * its launcher is run as it is, through the installer's own launch files and public names.
 */
public final class Boot112 {
    private Boot112() {}

    /** The property the platform mod finds the hand-over queue under. */
    public static final String HANDOVER = "horizon.bridge112.handover";

    private static volatile Function<String, String> commands;
    private static volatile String report;

    /**
     * @param mainClass Cleanroom's launcher, as its version.json names it
     * @param args the launcher's arguments (--tweakClass FML's, --gameDir, --assetsDir...)
     * @return the platform mod's report (JSON: mods, blocks, items, timings)
     */
    public static String start(String mainClass, String[] args) throws Exception {
        long started = System.nanoTime();
        SynchronousQueue<Object> handover = new SynchronousQueue<>();
        System.getProperties().put(HANDOVER, handover);
        Throwable[] failure = new Throwable[1];
        Thread thread = new Thread(() -> {
            try {
                Class.forName(mainClass).getMethod("main", String[].class).invoke(null, (Object) args);
                failure[0] = new IllegalStateException("Cleanroom's launcher returned before the platform mod took over (is horizon112 in mods/?)");
            } catch (Throwable t) {
                failure[0] = t;
            }
            handover.offer(failure[0]);
        }, "Horizon 1.12");
        thread.setDaemon(true);
        thread.start();
        Object first = handover.poll(10, TimeUnit.MINUTES);
        if (first == null) throw new IllegalStateException("Cleanroom didn't load in 10 minutes");
        if (first instanceof Throwable t) {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            throw new IllegalStateException("Cleanroom failed to load: " + root, t);
        }
        @SuppressWarnings("unchecked")
        Function<String, String> handler = (Function<String, String>) first;
        commands = handler;
        long ms = (System.nanoTime() - started) / 1_000_000;
        Runtime rt = Runtime.getRuntime();
        long used = (rt.totalMemory() - rt.freeMemory()) >> 20;
        report = handler.apply("report");
        log("Cleanroom ready in " + ms + " ms, " + used + " MB of heap in use");
        return "{\"start_ms\":" + ms + ",\"heap_mb\":" + used + "," + report.substring(1);
    }

    /**
     * Runs a platform command on 1.12's client thread and returns its answer: "report", "ping",
     * "screenshot MODID PATH" (a frame of the hidden client: its screen and MODID's item icons).
     */
    public static String call(String command) {
        Function<String, String> handler = commands;
        if (handler == null) throw new IllegalStateException("Cleanroom is not started");
        return handler.apply(command);
    }

    private static void log(String line) {
        try {
            Natives.log(line);
        } catch (UnsatisfiedLinkError e) {
            System.out.println("[horizon-112] " + line); // standalone (run112.sh)
        }
    }

    /** Standalone, as Rust would call it: {@code Boot112 <launcher> <args...>}, then the report. */
    public static void main(String[] args) throws Exception {
        String[] rest = java.util.Arrays.copyOfRange(args, 1, args.length);
        String out = start(args[0], rest);
        String path = System.getProperty("horizon.bridge112.report");
        if (path != null) java.nio.file.Files.writeString(java.nio.file.Path.of(path), out);
        log("report: " + out.length() + " chars" + (path != null ? " -> " + path : ""));
        log("ping: " + call("ping"));
        String shot = System.getProperty("horizon.bridge112.screenshot");
        if (shot != null) log("screenshot: " + call("screenshot " + System.getProperty("horizon.bridge112.screenshot.mod", "hello112") + " " + shot));
        System.exit(0);
    }
}
