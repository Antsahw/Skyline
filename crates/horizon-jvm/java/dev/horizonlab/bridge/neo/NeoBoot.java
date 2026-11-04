package dev.horizonlab.bridge.neo;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import dev.horizonlab.bridge.Natives;

/**
 * Starts NeoForge inside Horizon's JVM (prototype; docs/java-bridge-neoforge.md), the
 * counterpart of FabricBoot. On the system class path beside FancyModLoader and the game's
 * libraries, as the installer's args file lists them (horizon-jvm's neoforge.rs).
 *
 * FML's launcher ({@code net.neoforged.fml.startup.Server}) runs on a thread of its own: it
 * finds the mods, builds the game's class loader (Mixin, coremods, access transformers) and
 * calls the patched {@code net.minecraft.server.Main.main}. The platform mod's mixin there runs
 * vanilla's bootstrap and NeoForge's mod loading (construction, registration, common setup),
 * then calls {@link #ready} instead of starting a dedicated server, and parks that thread: FML
 * closes its loader when its launcher returns, so it never may.
 *
 * NeoForge and FML are LGPL: this runs them as they are, through their launcher and public
 * names only; nothing here comes from their source.
 */
public final class NeoBoot {
    private NeoBoot() {}

    private static final CountDownLatch READY = new CountDownLatch(1);
    private static volatile ClassLoader gameLoader;
    private static volatile Throwable failure;

    /**
     * @param mainClass FML's launcher, as the installer's args file names it
     * @param args its arguments (--fml.neoForgeVersion ... and --gameDir)
     * @return one line per loaded mod, "id version"
     */
    public static String[] start(String mainClass, String[] args) throws Exception {
        long started = System.nanoTime();
        Method main = Class.forName(mainClass).getMethod("main", String[].class);
        Thread thread = new Thread(() -> {
            try {
                main.invoke(null, (Object) args);
                if (READY.getCount() > 0) failure = new IllegalStateException("FML's launcher returned before the mods were loaded (is the platform mod in mods/?)");
            } catch (Throwable t) {
                failure = t;
            } finally {
                READY.countDown();
            }
        }, "Horizon NeoForge");
        thread.setDaemon(true);
        thread.start();
        if (!READY.await(10, TimeUnit.MINUTES)) throw new IllegalStateException("NeoForge didn't load in 10 minutes");
        if (failure != null) {
            Throwable root = failure;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            throw new IllegalStateException("NeoForge failed to load: " + root, failure);
        }
        List<String> mods = mods();
        log("NeoForge loaded " + mods.size() + " mods in " + (System.nanoTime() - started) / 1_000_000 + " ms");
        return mods.toArray(new String[0]);
    }

    /**
     * Called by the platform mod's mixin on FML's thread, once NeoForge has loaded the mods:
     * hands the game's class loader over and parks the thread for good.
     */
    public static void ready(ClassLoader loader) {
        gameLoader = loader;
        READY.countDown();
        while (true) {
            try {
                Thread.sleep(Long.MAX_VALUE);
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    /** A game or mod class through NeoForge's class loader (transformed, mixins applied). */
    public static Class<?> gameClass(String name) throws ClassNotFoundException {
        if (gameLoader == null) throw new IllegalStateException("NeoForge is not started");
        return Class.forName(name.replace('/', '.'), true, gameLoader);
    }

    /** "id version" of each mod, from FML's mod list (its public ModList and IModInfo). */
    private static List<String> mods() throws ReflectiveOperationException {
        Object list = Class.forName("net.neoforged.fml.ModList").getMethod("get").invoke(null);
        List<?> infos = (List<?>) list.getClass().getMethod("getMods").invoke(list);
        Class<?> info = Class.forName("net.neoforged.neoforgespi.language.IModInfo");
        Method id = info.getMethod("getModId");
        Method version = info.getMethod("getVersion");
        List<String> out = new ArrayList<>();
        for (Object mod : infos) out.add(id.invoke(mod) + " " + version.invoke(mod));
        return out;
    }

    private static void log(String line) {
        try {
            Natives.log(line);
        } catch (UnsatisfiedLinkError e) {
            System.out.println("[horizon-neo] " + line); // standalone (run_proto.sh)
        }
    }

    /** Standalone, as Rust would call it: {@code NeoBoot <launcher> <args...>}, then a report. */
    public static void main(String[] args) throws Exception {
        String[] rest = java.util.Arrays.copyOfRange(args, 1, args.length);
        for (String mod : start(args[0], rest)) log("mod " + mod);
        @SuppressWarnings("unchecked")
        List<String> lines = (List<String>) gameClass("dev.horizonlab.neoplatform.NeoCheck").getMethod("report").invoke(null);
        for (String line : lines) log(line);
        System.exit(0);
    }
}
