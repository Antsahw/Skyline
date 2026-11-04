package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.io.IOException;
import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Stream;

/**
 * Mods' client settings per player. One headless client serves every player, so a mod's
 * settings (one object in memory, one file in config/) were everyone's: a LAN guest changing
 * a mod's settings changed the host's. Two halves:
 *
 * <ul>
 *   <li>In memory: the objects holding mods' settings listed in holders() are swapped per player as the
 *       client changes player (HorizonClientState.load calls {@link #swap}). A player new to the
 *       client starts with a copy of the settings of the player before them (the host's first).
 *   <li>On disk: the first player the client serves owns config/ (the host, in a LAN world).
 *       After another player's input to a screen (where settings are saved), files that changed
 *       under config/ go to config/horizon-players/&lt;uuid&gt;/ and the owner's are put back.
 * </ul>
 *
 * A guest's saved settings last their session in memory; their kept files aren't read back at
 * their next join (no generic way to make a mod read its settings again).
 */
public final class HorizonClientConfig {
    private HorizonClientConfig() {}

    /** A field holding a settings object, on {@code target} (null for a static field). */
    private record Holder(String key, Object target, Field field) {}

    /** Each player's settings objects by holder key. */
    private static final Map<UUID, Map<String, Object>> SAVED = new ConcurrentHashMap<>();

    /** The settings holders there are now (AutoConfig's grow as mods register). */
    private static List<Holder> holders() {
        List<Holder> out = new ArrayList<>();
        return out;
    }

    private static final java.util.Set<String> FAILED = ConcurrentHashMap.newKeySet();

    private static void fail(String what, Throwable t) {
        if (FAILED.add(what)) Natives.log("Horizon's client: " + what + "'s settings can't be kept per player: " + t);
    }

    /**
     * The settings in memory become {@code to}'s: {@code from}'s are put away (null: nobody's
     * were in). A player with none yet gets a copy of the ones in memory.
     */
    static void swap(UUID from, UUID to) {
        if (owner == null) owner = to;
        List<Holder> all = holders();
        if (all.isEmpty()) return;
        try {
            if (from != null) {
                Map<String, Object> mine = SAVED.computeIfAbsent(from, k -> new HashMap<>());
                for (Holder h : all) mine.put(h.key(), h.field().get(h.target()));
            }
            // The first player keeps the settings as they are (nobody's were put away before).
            if (from == null && !SAVED.containsKey(to)) return;
            Map<String, Object> theirs = SAVED.get(to);
            // One copy for an object two holders hold.
            IdentityHashMap<Object, Object> done = new IdentityHashMap<>();
            for (Holder h : all) {
                Object current = h.field().get(h.target());
                Object value = theirs != null && theirs.containsKey(h.key()) ? theirs.get(h.key()) : copy(current, done);
                if (value != current) h.field().set(h.target(), value);
            }
        } catch (ReflectiveOperationException | RuntimeException e) {
            fail("swap", e);
        }
    }

    static void forget(UUID player) {
        SAVED.remove(player);
    }

    // ---- Files ----------------------------------------------------------------------------

    /** Who owns config/: the first player the client served (the host). */
    private static UUID owner;
    /** config/'s files as the owner left them: contents and times. */
    private static Map<Path, byte[]> base;
    private static Map<Path, FileTime> baseTimes;
    private static final String PLAYERS = "horizon-players";

    private static Path dir() {
        return net.fabricmc.loader.api.FabricLoader.getInstance().getConfigDir();
    }

    /**
     * After {@code player}'s input to a screen: the owner's changes become the base; another
     * player's changed files are moved to their own folder and the owner's put back.
     */
    static synchronized void afterInput(UUID player) {
        if (player == null) return;
        if (owner == null) owner = player;
        try {
            if (base == null || player.equals(owner)) {
                snapshot();
                return;
            }
            divert(player);
        } catch (IOException | RuntimeException e) {
            fail("config files", e);
        }
    }

    private static Stream<Path> files() throws IOException {
        Path dir = dir();
        if (!Files.isDirectory(dir)) return Stream.empty();
        Path players = dir.resolve(PLAYERS);
        return Files.walk(dir).filter(Files::isRegularFile).filter(p -> !p.startsWith(players));
    }

    private static void snapshot() throws IOException {
        Map<Path, byte[]> contents = new HashMap<>();
        Map<Path, FileTime> times = new HashMap<>();
        try (var files = files()) {
            for (Path p : (Iterable<Path>) files::iterator) {
                FileTime time = Files.getLastModifiedTime(p);
                // Unchanged since the last look: kept as read.
                if (base != null && time.equals(baseTimes.get(p)) && base.containsKey(p)) {
                    contents.put(p, base.get(p));
                } else if (Files.size(p) <= 4 << 20) {
                    contents.put(p, Files.readAllBytes(p));
                }
                times.put(p, time);
            }
        }
        base = contents;
        baseTimes = times;
    }

    private static void divert(UUID player) throws IOException {
        Path dir = dir();
        Path mine = dir.resolve(PLAYERS).resolve(player.toString());
        List<Path> changed = new ArrayList<>();
        try (var files = files()) {
            for (Path p : (Iterable<Path>) files::iterator) {
                if (!Files.getLastModifiedTime(p).equals(baseTimes.get(p))) changed.add(p);
            }
        }
        for (Path p : changed) {
            Path to = mine.resolve(dir.relativize(p).toString());
            Files.createDirectories(to.getParent());
            Files.copy(p, to, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            byte[] was = base.get(p);
            if (was != null) {
                Files.write(p, was);
                Files.setLastModifiedTime(p, baseTimes.get(p));
            } else if (!baseTimes.containsKey(p)) {
                Files.delete(p); // New with this player's save: theirs only.
            }
        }
        if (!changed.isEmpty() && DEBUG) Natives.log("Horizon's client: " + changed.size() + " settings file(s) kept for " + player);
    }

    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("screens");

    // ---- Copies ---------------------------------------------------------------------------

    /**
     * A deep copy of a settings object: its own classes' objects, arrays and collections are
     * copied; the game's, the JDK's and libraries' objects (registries, items, identifiers,
     * codecs) and immutable values are shared.
     */
    static Object copy(Object value) {
        return copy(value, new IdentityHashMap<>());
    }

    private static Object copy(Object value, IdentityHashMap<Object, Object> done) {
        if (value == null) return null;
        Object seen = done.get(value);
        if (seen != null) return seen;
        Class<?> type = value.getClass();
        if (shared(type)) return value;
        if (type.isArray()) {
            int n = Array.getLength(value);
            Object out = Array.newInstance(type.getComponentType(), n);
            done.put(value, out);
            for (int i = 0; i < n; i++) Array.set(out, i, type.getComponentType().isPrimitive() ? Array.get(value, i) : copy(Array.get(value, i), done));
            return out;
        }
        if (value instanceof Collection<?> c) return copyCollection(c, done);
        if (value instanceof Map<?, ?> m) return copyMap(m, done);
        if (type.getName().startsWith("java.") || type.getName().startsWith("javax.")) return value;
        try {
            Object out = Natives.allocate(type);
            done.put(value, out);
            for (Class<?> c = type; c != null && c != Object.class; c = c.getSuperclass()) {
                for (Field f : c.getDeclaredFields()) {
                    if (Modifier.isStatic(f.getModifiers())) continue;
                    f.setAccessible(true);
                    Object v = f.get(value);
                    f.set(out, f.getType().isPrimitive() ? v : copy(v, done));
                }
            }
            return out;
        } catch (Throwable t) {
            return value;
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Object copyCollection(Collection<?> c, IdentityHashMap<Object, Object> done) {
        try {
            Collection out = (Collection) c.getClass().getDeclaredConstructor().newInstance();
            done.put(c, out);
            for (Object e : c) out.add(copy(e, done));
            return out;
        } catch (ReflectiveOperationException | RuntimeException e) {
            return c; // Immutable (List.of) or unknown: shared.
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Object copyMap(Map<?, ?> m, IdentityHashMap<Object, Object> done) {
        try {
            Map out = (Map) m.getClass().getDeclaredConstructor().newInstance();
            done.put(m, out);
            for (var e : m.entrySet()) out.put(e.getKey(), copy(e.getValue(), done));
            return out;
        } catch (ReflectiveOperationException | RuntimeException e) {
            return m;
        }
    }

    /** Values a copy shares: immutable ones, and objects that aren't a mod's own. */
    private static boolean shared(Class<?> type) {
        if (type.isPrimitive() || type.isEnum() || type.isRecord() || Number.class.isAssignableFrom(type)
                || type == String.class || type == Boolean.class || type == Character.class || type == Class.class) {
            return true;
        }
        String name = type.getName();
        return name.startsWith("net.minecraft.") || name.startsWith("com.mojang.") || name.startsWith("com.google.")
                || name.startsWith("it.unimi.") || name.startsWith("org.joml.") || name.startsWith("net.fabricmc.")
                || name.startsWith("org.slf4j.") || name.startsWith("org.apache.") || name.startsWith("dev.horizonlab.")
                || java.util.function.Function.class.isAssignableFrom(type) || type.isSynthetic() || type.isHidden();
    }
}
