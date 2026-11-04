package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;
import java.util.TreeSet;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.loader.api.FabricLoader;
import org.spongepowered.asm.mixin.transformer.meta.MixinMerged;

/**
 * Mods whose client half uses the player's own sound devices: it opens a microphone or a
 * speaker of its own with OpenAL (Simple Voice Chat) or Java's sound API. Their client half must
 * run on the machine of the player it is for: a LAN host's or a dedicated server's headless
 * client runs every player's client, and would open the host's microphone for each guest (and
 * such mods keep one client per process: a second player's join replaced the first's).
 *
 * <p>So, on a server, these mods' client code runs only for the player whose game this process
 * is (the window's own player: single player, a LAN host); for everyone else it is left out
 * (their client events, ticks, key handlers, HUD), and their messages on these mods' channels go
 * to and from that player's own game, which runs the mods' client half itself
 * (HorizonDeviceClient: a game connected to another's server). Found by what the mods' classes
 * call, not by name: any mod that takes the microphone or opens a speaker device.
 */
public final class HorizonDeviceMods {
    private HorizonDeviceMods() {}

    /** What a class calls to take a device (in its constant pool). */
    private static final String[] DEVICE_CALLS = {
        "alcCaptureOpenDevice", "alcOpenDevice", "javax/sound/sampled/TargetDataLine",
    };

    private static volatile Set<String> mods;
    private static Thread scan;

    /** The player whose game this process is (Rust: the single-player world's owner), or null. */
    static volatile UUID own;

    /** Players whose own game runs these mods' client half (it said so as it joined). */
    static final Set<UUID> RUN_THEIR_OWN = ConcurrentHashMap.newKeySet();

    /** The scan starts beside the client's start (HorizonClient.finish). */
    static synchronized void startScan() {
        if (scan != null || mods != null) return;
        scan = new Thread(() -> mods = scanNow(), "Horizon device mods");
        scan.setDaemon(true);
        scan.start();
    }

    /** The device mods' ids (empty: none). Waits for the scan the first time. */
    public static Set<String> mods() {
        Set<String> found = mods;
        if (found != null) return found;
        Thread running;
        synchronized (HorizonDeviceMods.class) {
            running = scan;
        }
        if (running != null) {
            try {
                running.join();
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }
        synchronized (HorizonDeviceMods.class) {
            if (mods == null) mods = scanNow();
            return mods;
        }
    }

    private static Set<String> scanNow() {
        Set<String> out = new TreeSet<>();
        byte[][] needles = new byte[DEVICE_CALLS.length][];
        for (int i = 0; i < needles.length; i++) needles[i] = DEVICE_CALLS[i].getBytes(StandardCharsets.UTF_8);
        for (var mod : FabricLoader.getInstance().getAllMods()) {
            String id = mod.getMetadata().getId();
            if (id.equals("minecraft") || id.equals("java") || id.equals("horizon") || id.equals("fabricloader")
                    || id.startsWith("fabric-") || id.equals("fabric-api") || id.equals("mixinextras")) {
                continue;
            }
            // Client code only: a mod with no client entrypoint or client mixins has no client half.
            try {
                for (Path root : mod.getRootPaths()) {
                    if (takesDevice(root, needles)) {
                        out.add(id);
                        break;
                    }
                }
            } catch (Throwable t) {
                Natives.log("Java mods: couldn't read " + id + "'s classes for device use: " + t);
            }
        }
        if (!out.isEmpty()) {
            Natives.log("Java mods: " + String.join(", ", out) + " use the player's own sound devices: their client half"
                    + " runs in each player's own game");
        }
        return java.util.Collections.unmodifiableSet(out);
    }

    private static boolean takesDevice(Path root, byte[][] needles) throws java.io.IOException {
        try (var files = Files.walk(root)) {
            for (Path file : (Iterable<Path>) files::iterator) {
                String name = file.toString();
                // Libraries the mod carries (shaded: concentus, lame...) count as the mod's.
                if (!name.endsWith(".class") || name.contains("META-INF")) continue;
                byte[] bytes = Files.readAllBytes(file);
                for (byte[] needle : needles) {
                    if (contains(bytes, needle)) return true;
                }
            }
        }
        return false;
    }

    private static boolean contains(byte[] hay, byte[] needle) {
        outer:
        for (int i = 0, end = hay.length - needle.length; i <= end; i++) {
            for (int j = 0; j < needle.length; j++) {
                if (hay[i + j] != needle[j]) continue outer;
            }
            return true;
        }
        return false;
    }

    /** Whether a mod's id is a device mod's. */
    public static boolean device(String mod) {
        return mod != null && !mods().isEmpty() && mods().contains(mod);
    }

    /**
     * Whether {@code mod}'s client code runs now: in a device client (HorizonDeviceClient) only
     * the device mods' does; on a server, a device mod's runs only for this process's own player
     * (or when no player's client runs).
     */
    public static boolean runs(String mod) {
        if (mods().isEmpty()) return true;
        boolean device = mod != null && mods().contains(mod);
        if (HorizonDeviceClient.on()) return device;
        if (!device) return true;
        var player = net.minecraft.client.Minecraft.getInstance() == null ? null : net.minecraft.client.Minecraft.getInstance().player;
        boolean runs = player == null || player.getUUID().equals(own);
        if (DEBUG && SAID.add(mod + " " + (player == null ? null : player.getUUID()) + " " + runs)) {
            Natives.log("device mods: " + mod + (runs ? " runs" : " is left out") + " for " + (player == null ? "no player" : player.getUUID()));
        }
        return runs;
    }

    /** HORIZON_JAVA_DEBUG=device: each decision is logged once. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("device");
    private static final Set<String> SAID = ConcurrentHashMap.newKeySet();

    /** {@link #runs} for code of a class (a listener, a lambda's class). */
    public static boolean runs(Class<?> owner) {
        if (mods().isEmpty() && !HorizonDeviceClient.on()) return true;
        String name = owner.getName();
        int lambda = name.indexOf("$$");
        String mod = HorizonFailures.modOf(lambda > 0 ? name.substring(0, lambda) : name);
        if (DEBUG && SAID.add(name)) Natives.log("device mods: " + name + " is " + mod + "'s");
        return runs(mod);
    }

    /** {@link #runs} for a mixin's handler merged into a game class. */
    public static boolean runs(Method handler) {
        if (mods().isEmpty() && !HorizonDeviceClient.on()) return true;
        MixinMerged merged = handler.getAnnotation(MixinMerged.class);
        if (merged == null) return runs(handler.getDeclaringClass());
        return runs(HorizonFailures.modOf(merged.mixin()));
    }

    /** The mod a payload channel is for: its namespace when that is a device mod's id. */
    static boolean deviceChannel(net.minecraft.resources.Identifier channel) {
        return device(channel.getNamespace());
    }
}
