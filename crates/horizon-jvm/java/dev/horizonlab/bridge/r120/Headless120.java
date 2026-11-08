package dev.horizonlab.bridge.r120;

import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;

/**
 * The 1.20.1 client mods' code reaches, without a window (the start of what the 26.3 bridge's
 * HorizonClient is): a {@code Minecraft} made without its constructor (which would open the
 * window and start the game), set as {@code Minecraft.getInstance()}, in the order vanilla's
 * constructor fills it: the game folder and the player ({@link #make}, before Fabric's
 * entrypoints), then real {@code Options} ({@link #options}, after the client entrypoints:
 * mods register key mappings at start and Fabric API refuses them once Options exist).
 * Whatever else a mod's client start needs is found by the boot report and added here as the
 * phases go.
 */
final class Headless120 {
    private Headless120() {}

    /** Makes the client; returns what failed, or {@code null}. */
    static String make(Names120 n, ClassLoader game, Path gameDir) {
        try {
            Class<?> minecraftClass = n.cls("net.minecraft.client.Minecraft", game);
            Object minecraft = allocate(minecraftClass);
            n.field(minecraftClass, "instance").set(null, minecraft);
            File dir = gameDir.toFile();
            set(n, minecraftClass, minecraft, "gameDirectory", dir);
            // Its event loop's task queue (BlockableEventLoop's constructor makes it): mods
            // hand tasks to the client thread with execute(). They wait there until the
            // client ticks (a later phase runs them).
            set(n, minecraftClass, minecraft, "pendingRunnables", new java.util.concurrent.ConcurrentLinkedQueue<Runnable>());
            set(n, minecraftClass, minecraft, "name", "Client");
            // An offline player, as a launcher passes one: name, UUID, token, xuid, client id, type.
            Class<?> userClass = n.cls("net.minecraft.client.User", game);
            Class<?> typeClass = n.cls("net.minecraft.client.User$Type", game);
            Object legacy = n.callStatic(typeClass, "byName", "legacy");
            Object user = userClass.getConstructor(String.class, String.class, String.class, java.util.Optional.class, java.util.Optional.class, typeClass)
                    .newInstance("Horizon", "00000000000000000000000000000000", "0", java.util.Optional.empty(), java.util.Optional.empty(), legacy);
            set(n, minecraftClass, minecraft, "user", user);
            return null;
        } catch (Throwable e) {
            return Boot120.root(e);
        }
    }

    /** The client's Options, once the client entrypoints ran; returns what failed, or {@code null}. */
    static String options(Names120 n, ClassLoader game, Path gameDir) {
        try {
            Class<?> minecraftClass = n.cls("net.minecraft.client.Minecraft", game);
            Object minecraft = n.getStatic(minecraftClass, "instance");
            Class<?> optionsClass = n.cls("net.minecraft.client.Options", game);
            Object options = optionsClass.getConstructor(minecraftClass, File.class).newInstance(minecraft, gameDir.toFile());
            set(n, minecraftClass, minecraft, "options", options);
            return null;
        } catch (Throwable e) {
            return Boot120.root(e);
        }
    }

    private static void set(Names120 n, Class<?> type, Object target, String field, Object value) throws ReflectiveOperationException {
        Field f = n.field(type, field);
        f.set(target, value);
    }

    /** An object of {@code type} without running a constructor (sun.misc.Unsafe, jdk.unsupported). */
    static Object allocate(Class<?> type) throws ReflectiveOperationException {
        Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
        Field theUnsafe = unsafeClass.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        Method allocateInstance = unsafeClass.getMethod("allocateInstance", Class.class);
        return allocateInstance.invoke(theUnsafe.get(null), type);
    }
}
