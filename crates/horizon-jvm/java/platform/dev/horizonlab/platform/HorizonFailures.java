package dev.horizonlab.platform;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * A mod's failure, told to the players in chat (which mod, where, what was thrown), not only
 * written to the log: "[Java mods] Farmer's Delight failed in the HUD: NullPointerException: …
 * (the game goes on; the log has more)". The mod is found by the failure's stack (the first
 * frame in a mod's jar, past the game's, Fabric's and Horizon's), or by the mixin a failing
 * handler came from. Each mod and place is told once a session, at most {@link #LIMIT} in all.
 */
public final class HorizonFailures {
    private HorizonFailures() {}

    private static final int LIMIT = 8;
    private static final Set<String> TOLD = ConcurrentHashMap.newKeySet();
    private static volatile boolean overLimit;

    /** Mods' code roots (their jars or folders, as URLs' text) and their ids. */
    private static volatile List<String[]> roots;
    private static final Map<String, String> BY_CLASS = new ConcurrentHashMap<>();
    /** Mods never blamed: the game, the loader, its API, the bridge. */
    private static final Set<String> NOT_BLAMED = Set.of("minecraft", "java", "fabricloader", "fabric-api", "horizon", "mixinextras");

    /** A mod failed {@code where} (a short phrase: "the HUD", "a tick"): its players are told. */
    public static void report(String where, Throwable t) {
        report(where, t, null);
    }

    /** As {@link #report(String, Throwable)}, the failing code being {@code handler} (a mixin's). */
    public static void report(String where, Throwable t, Method handler) {
        try {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            // "Event [listener class]" names: the event says where; the mod is found below.
            int bracket = where.indexOf(" [");
            if (bracket > 0) where = where.substring(0, bracket);
            String mod = null;
            if (handler != null) {
                var merged = handler.getAnnotation(org.spongepowered.asm.mixin.transformer.meta.MixinMerged.class);
                if (merged != null) mod = modOf(merged.mixin());
                if (mod == null) mod = modOf(handler.getDeclaringClass().getName());
            }
            if (mod == null) mod = modOf(root);
            if (mod == null && root != t) mod = modOf(t);
            String key = (mod == null ? "?" : mod) + "|" + where;
            if (!TOLD.add(key)) return;
            if (TOLD.size() > LIMIT) {
                if (!overLimit) {
                    overLimit = true;
                    tell(line("More Java mods failed: the log has them.", null, null));
                }
                return;
            }
            String name = mod == null ? "A Java mod" : name(mod);
            String what = root.getClass().getSimpleName() + (root.getMessage() == null ? "" : ": " + trim(root.getMessage()));
            tell(line(name, " failed in " + where + ": " + what, " (the game goes on; the log has more)"));
        } catch (Throwable ignored) {
            // Telling is best effort: the log has the failure.
        }
    }

    /**
     * A mod Horizon left out because it isn't needed (a renderer: Horizon has its own): told
     * as a note, not a failure, and not counted in {@link #LIMIT}.
     */
    static void notNeeded(String text) {
        JsonObject o = new JsonObject();
        o.addProperty("text", "");
        JsonArray extra = new JsonArray();
        extra.add(part("[Java mods] ", "gold"));
        extra.add(part(text, "gray"));
        o.add("extra", extra);
        tell(o);
    }

    private static String trim(String message) {
        String m = message.replace('\n', ' ');
        return m.length() > 120 ? m.substring(0, 117) + "..." : m;
    }

    private static JsonObject line(String name, String what, String tail) {
        JsonObject o = new JsonObject();
        o.addProperty("text", "");
        JsonArray extra = new JsonArray();
        extra.add(part("[Java mods] ", "gold"));
        extra.add(part(name, what == null ? "red" : "yellow"));
        if (what != null) extra.add(part(what, "red"));
        if (tail != null) extra.add(part(tail, "gray"));
        o.add("extra", extra);
        return o;
    }

    private static JsonObject part(String text, String color) {
        JsonObject o = new JsonObject();
        o.addProperty("text", text);
        o.addProperty("color", color);
        return o;
    }

    /** Messages from before any player joined: told to the first players there are. */
    private static final List<String> PENDING = java.util.Collections.synchronizedList(new ArrayList<>());

    /** To every player on the server (kept until there is one). */
    private static void tell(JsonObject message) {
        String json = message.toString();
        var players = HorizonServer.players();
        if (players.isEmpty()) {
            PENDING.add(json);
            return;
        }
        for (var player : players) Natives.tell(player.getStringUUID(), json);
    }

    /** A player is in: what failed before anyone was is told now. */
    static void flush() {
        if (PENDING.isEmpty()) return;
        var players = HorizonServer.players();
        if (players.isEmpty()) return;
        List<String> pending;
        synchronized (PENDING) {
            pending = new ArrayList<>(PENDING);
            PENDING.clear();
        }
        for (String json : pending) {
            for (var player : players) Natives.tell(player.getStringUUID(), json);
        }
    }

    private static String name(String mod) {
        return net.fabricmc.loader.api.FabricLoader.getInstance().getModContainer(mod)
                .map(c -> c.getMetadata().getName()).orElse(mod);
    }

    /** The mod whose code a stack is in: its first frame in a mod's jar. */
    private static String modOf(Throwable t) {
        for (StackTraceElement frame : t.getStackTrace()) {
            String mod = modOf(frame.getClassName());
            if (mod != null) return mod;
        }
        return null;
    }

    /** The mod a class (by name) comes from, null for the game, Fabric and Horizon. */
    static String modOf(String className) {
        if (className.startsWith("java.") || className.startsWith("jdk.") || className.startsWith("sun.")
                || className.startsWith("net.minecraft.") || className.startsWith("com.mojang.")
                || className.startsWith("dev.horizonlab.platform.") || className.startsWith("dev.horizonlab.bridge.")
                || className.startsWith("net.fabricmc.") || className.startsWith("org.spongepowered.")) {
            return null;
        }
        String cached = BY_CLASS.get(className);
        if (cached != null) return cached.isEmpty() ? null : cached;
        String mod = null;
        var url = HorizonFailures.class.getClassLoader().getResource(className.replace('.', '/') + ".class");
        if (url != null) {
            // (A URL's "+" is a plus, not a form's space: most mods' jars have one in their name,
            // voicechat-fabric-2.6.27+26.3.jar, and were blamed on no one.)
            String where = java.net.URLDecoder.decode(url.toString().replace("+", "%2B"), java.nio.charset.StandardCharsets.UTF_8);
            for (String[] root : roots()) {
                if (where.contains(root[0])) {
                    mod = root[1];
                    break;
                }
            }
        }
        BY_CLASS.put(className, mod == null ? "" : mod);
        return mod;
    }

    private static List<String[]> roots() {
        if (roots != null) return roots;
        List<String[]> out = new ArrayList<>();
        for (var container : net.fabricmc.loader.api.FabricLoader.getInstance().getAllMods()) {
            String id = container.getMetadata().getId();
            if (NOT_BLAMED.contains(id) || id.startsWith("fabric-")) continue;
            try {
                for (var path : container.getOrigin().getPaths()) {
                    out.add(new String[] {path.toAbsolutePath().toString(), id});
                }
            } catch (RuntimeException e) {
                // A mod with no paths of its own (nested in another's): its parent is blamed.
            }
        }
        // The longest roots first: a nested jar inside its parent's folder is its own.
        out.sort((a, b) -> b[0].length() - a[0].length());
        roots = out;
        return out;
    }
}
