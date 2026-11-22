package dev.horizonlab.platform120;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import org.spongepowered.asm.mixin.transformer.meta.MixinMerged;

/**
 * The mods' mixins on vanilla behaviour that Horizon runs itself (docs/java-bridge-120.md,
 * "Mixins on vanilla behaviour"). Horizon never calls 1.20.1's {@code Player.tick} or
 * {@code LivingEntity.hurt}, so what mods inject there doesn't run: mostly library mods
 * (Architectury, Collective, Puzzles Lib, Balm) turning those methods into events for the mods
 * built on them. When Horizon gets to such a moment it asks the runtime to {@link #fire} the
 * method's injected handlers on the mirror's object, with the method's arguments.
 *
 * <p>Which handler belongs to which method isn't kept at run time: the mixin classes' own
 * annotations say it ({@code @Inject(method = ...)}), so each mod's mixin configs, its refmap
 * and its mixin classes' bytes are read once ({@link #index}), and the method named as Mojang
 * does ({@link Names}). Only {@code @Inject} handlers can be called this way (they take the
 * method's arguments and a {@link CallbackInfo}); a {@code @Redirect} or {@code @ModifyArg}
 * changes code inside the method, which Horizon's own version doesn't have.
 */
public final class Mixins120 {
    private Mixins120() {}

    /** A handler: the mixin class (dotted) and the handler's name in it. */
    record Handler(String mixin, String name) {}

    /** Mojang's {@code owner.method} to its @Inject handlers. */
    private static Map<String, List<Handler>> index;
    private static final Map<String, Method[]> FOUND = new ConcurrentHashMap<>();
    private static final Set<String> FAILED = ConcurrentHashMap.newKeySet();
    /** The methods whose handlers have run, each said once in the runtime's log. */
    private static final Set<String> FIRED = ConcurrentHashMap.newKeySet();
    private static final String INJECT = "Lorg/spongepowered/asm/mixin/injection/Inject;";
    private static final String MIXIN = "Lorg/spongepowered/asm/mixin/Mixin;";

    /** What firing a method's handlers came to. */
    public record Result(int ran, boolean cancelled, Object returned) {}

    /** The handlers mods inject into {@code target} (Mojang's {@code net.minecraft....Player.tick}). */
    static List<Handler> handlers(String target) {
        return index().getOrDefault(target, List.of());
    }

    /**
     * Runs every handler mods injected into `target` on `self` (the mirror's object; null for a
     * static method) with the method's `args`. `returnable`: the method returns a value, so the
     * handlers get a {@link CallbackInfoReturnable}. Fabric API's own are left out: Events120
     * fires its events. A handler that fails is logged once and skipped.
     */
    public static Result fire(String target, Object self, Object[] args, boolean returnable) {
        List<Handler> handlers = handlers(target);
        if (handlers.isEmpty()) return new Result(0, false, null);
        int ran = 0;
        boolean cancelled = false;
        Object returned = null;
        String method = target.substring(target.lastIndexOf('.') + 1);
        for (Handler h : handlers) {
            if (h.mixin().startsWith("net.fabricmc.") || h.mixin().startsWith("dev.horizonlab.")) continue;
            Method m = merged(self == null ? null : self.getClass(), h);
            if (m == null) continue;
            Class<?>[] params = m.getParameterTypes();
            if (params.length != args.length + 1) continue;
            Object info = returnable
                    ? new CallbackInfoReturnable<Object>(method, true)
                    : new CallbackInfo(method, true);
            if (!params[args.length].isInstance(info)) continue;
            Object[] call = new Object[args.length + 1];
            System.arraycopy(args, 0, call, 0, args.length);
            call[args.length] = info;
            try {
                m.setAccessible(true);
                m.invoke(Modifier.isStatic(m.getModifiers()) ? null : self, call);
                ran++;
                if (((CallbackInfo) info).isCancelled()) {
                    cancelled = true;
                    if (info instanceof CallbackInfoReturnable<?> r) returned = r.getReturnValue();
                    break;
                }
            } catch (Throwable e) {
                Throwable cause = e.getCause() != null ? e.getCause() : e;
                if (FAILED.add(h.mixin() + "." + h.name())) {
                    Mirror120.log("a mod's handler on " + target + " (" + h.mixin() + "." + h.name()
                            + ") failed: " + cause);
                }
            }
        }
        if (ran > 0 && FIRED.add(target)) {
            Mirror120.log("the mods' handlers on " + target + ": " + ran + " ran");
        }
        return new Result(ran, cancelled, returned);
    }

    /** The handler as merged into `type` or a supertype: Mixin renames it, keeping its name last. */
    private static Method merged(Class<?> type, Handler h) {
        String key = (type == null ? "" : type.getName()) + "|" + h.mixin() + "|" + h.name();
        Method[] cached = FOUND.get(key);
        if (cached != null) return cached.length == 0 ? null : cached[0];
        Method found = null;
        for (Class<?> c = type; c != null && found == null; c = c.getSuperclass()) {
            for (Method m : c.getDeclaredMethods()) {
                MixinMerged merged = m.getAnnotation(MixinMerged.class);
                if (merged == null || !merged.mixin().equals(h.mixin())) continue;
                if (m.getName().equals(h.name()) || m.getName().endsWith("$" + h.name())) {
                    found = m;
                    break;
                }
            }
        }
        FOUND.put(key, found == null ? new Method[0] : new Method[] {found});
        return found;
    }

    // ------------------------------------------------------------------------------------------
    // The index: every mod's @Inject handlers by the vanilla method they inject into.

    static synchronized Map<String, List<Handler>> index() {
        if (index != null) return index;
        index = new HashMap<>();
        for (var mod : Loader120.get().modMixins()) {
            try {
                indexMod(mod);
            } catch (Throwable e) {
                Mirror120.log("a mod's mixins couldn't be read: " + e);
            }
        }
        int handlers = index.values().stream().mapToInt(List::size).sum();
        Mirror120.log("the mods' mixins: " + handlers + " @Inject handlers on " + index.size() + " vanilla methods");
        return index;
    }

    private static JsonObject json(Loader120.ModMixins mod, String path) {
        Optional<byte[]> bytes = mod.read().apply(path);
        if (bytes.isEmpty()) return null;
        try {
            JsonElement e = JsonParser.parseString(new String(bytes.get(), StandardCharsets.UTF_8));
            return e.isJsonObject() ? e.getAsJsonObject() : null;
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static void indexMod(Loader120.ModMixins mod) {
        for (String config : mod.configs()) {
            JsonObject cfg = json(mod, config);
            if (cfg == null) continue;
            JsonObject refmap = null;
            if (cfg.has("refmap")) {
                JsonObject all = json(mod, cfg.get("refmap").getAsString());
                if (all != null && all.has("mappings")) refmap = all.getAsJsonObject("mappings");
            }
            String pkg = cfg.has("package") ? cfg.get("package").getAsString() : "";
            for (String list : new String[] {"mixins", "client", "server"}) {
                if (!cfg.has(list) || !cfg.get(list).isJsonArray()) continue;
                for (JsonElement name : cfg.getAsJsonArray(list)) {
                    String mixin = pkg + "." + name.getAsString();
                    Optional<byte[]> bytes = mod.read().apply(mixin.replace('.', '/') + ".class");
                    if (bytes.isEmpty()) continue;
                    JsonObject own = refmap != null && refmap.has(mixin.replace('.', '/'))
                            ? refmap.getAsJsonObject(mixin.replace('.', '/')) : null;
                    try {
                        indexMixin(mixin, bytes.get(), own);
                    } catch (RuntimeException e) {
                        // A class this reader can't follow: its handlers stay unknown.
                    }
                }
            }
        }
    }

    private static void indexMixin(String mixin, byte[] bytes, JsonObject refmap) {
        ClassAnnotations parsed = ClassAnnotations.read(bytes);
        if (parsed == null) return;
        List<String> owners = new ArrayList<>();
        for (var a : parsed.annotations) {
            if (!a.type().equals(MIXIN)) continue;
            for (Object v : list(a.values().get("value"))) {
                if (v instanceof String s) owners.add(s.startsWith("L") ? s.substring(1, s.length() - 1) : s);
            }
            for (Object v : list(a.values().get("targets"))) {
                if (v instanceof String s) owners.add(mapped(refmap, s).replace('.', '/'));
            }
        }
        for (var method : parsed.methods) {
            for (var a : method.annotations()) {
                if (!a.type().equals(INJECT)) continue;
                for (Object t : list(a.values().get("method"))) {
                    if (!(t instanceof String target)) continue;
                    for (String key : targets(mapped(refmap, target), owners)) {
                        index.computeIfAbsent(key, k -> new ArrayList<>()).add(new Handler(mixin, method.name()));
                    }
                }
            }
        }
    }

    private static String mapped(JsonObject refmap, String s) {
        return refmap != null && refmap.has(s) ? refmap.get(s).getAsString() : s;
    }

    private static List<Object> list(Object v) {
        if (v == null) return List.of();
        if (v instanceof List<?> l) return new ArrayList<>(l);
        return List.of(v);
    }

    /** {@code Lowner;name(desc)} or {@code name(desc)} or {@code name}, as Mojang's {@code owner.name}. */
    private static List<String> targets(String target, List<String> owners) {
        String owner = null, rest = target.trim();
        if (rest.startsWith("L") && rest.indexOf(';') > 0) {
            owner = rest.substring(1, rest.indexOf(';'));
            rest = rest.substring(rest.indexOf(';') + 1);
        }
        int paren = rest.indexOf('(');
        String name = paren >= 0 ? rest.substring(0, paren) : rest;
        if (name.isEmpty() || name.contains("*")) return List.of();
        List<String> out = new ArrayList<>();
        for (String o : owner != null ? List.of(owner) : owners) {
            out.add(Names.mojangClass(o) + "." + Names.mojangMethod(name));
        }
        return out;
    }

    // ------------------------------------------------------------------------------------------
    // Class files: a class's and its methods' annotations, invisible ones included (Mixin's are).

    record Annotation(String type, Map<String, Object> values) {}

    record MethodAnnotations(String name, String desc, List<Annotation> annotations) {}

    static final class ClassAnnotations {
        final List<Annotation> annotations = new ArrayList<>();
        final List<MethodAnnotations> methods = new ArrayList<>();
        private final java.nio.ByteBuffer d;
        private Object[] pool;

        private ClassAnnotations(byte[] bytes) {
            d = java.nio.ByteBuffer.wrap(bytes);
        }

        static ClassAnnotations read(byte[] bytes) {
            ClassAnnotations c = new ClassAnnotations(bytes);
            try {
                return c.parse() ? c : null;
            } catch (RuntimeException e) {
                return null;
            }
        }

        private int u1() {
            return d.get() & 0xFF;
        }

        private int u2() {
            return d.getShort() & 0xFFFF;
        }

        private String utf(int i) {
            return i > 0 && i < pool.length && pool[i] instanceof String s ? s : null;
        }

        private boolean parse() {
            if (d.getInt() != 0xCAFEBABE) return false;
            d.getInt();
            int count = u2();
            pool = new Object[count];
            for (int i = 1; i < count; i++) {
                int tag = u1();
                switch (tag) {
                    case 1 -> {
                        byte[] b = new byte[u2()];
                        d.get(b);
                        pool[i] = new String(b, StandardCharsets.UTF_8);
                    }
                    case 3, 4 -> pool[i] = d.getInt();
                    case 5, 6 -> {
                        d.getLong();
                        i++;
                    }
                    case 7, 8, 16, 19, 20 -> pool[i] = new int[] {tag, u2()};
                    case 9, 10, 11, 12, 17, 18 -> {
                        u2();
                        u2();
                    }
                    case 15 -> {
                        u1();
                        u2();
                    }
                    default -> {
                        return false;
                    }
                }
            }
            u2();
            u2();
            u2();
            int interfaces = u2();
            d.position(d.position() + 2 * interfaces);
            int fields = u2();
            for (int i = 0; i < fields; i++) {
                d.position(d.position() + 6);
                attributes();
            }
            int methods = u2();
            for (int i = 0; i < methods; i++) {
                u2();
                String name = utf(u2()), desc = utf(u2());
                this.methods.add(new MethodAnnotations(name, desc, attributes()));
            }
            annotations.addAll(attributes());
            return true;
        }

        private List<Annotation> attributes() {
            List<Annotation> found = new ArrayList<>();
            int count = u2();
            for (int i = 0; i < count; i++) {
                String name = utf(u2());
                int length = d.getInt();
                int end = d.position() + length;
                if ("RuntimeInvisibleAnnotations".equals(name) || "RuntimeVisibleAnnotations".equals(name)) {
                    int n = u2();
                    for (int k = 0; k < n; k++) found.add(annotation());
                }
                d.position(end);
            }
            return found;
        }

        private Annotation annotation() {
            String type = utf(u2());
            Map<String, Object> values = new HashMap<>();
            int n = u2();
            for (int i = 0; i < n; i++) {
                String name = utf(u2());
                values.put(name, value());
            }
            return new Annotation(type, values);
        }

        private Object value() {
            char tag = (char) u1();
            switch (tag) {
                case 's' -> {
                    return utf(u2());
                }
                case 'B', 'C', 'D', 'F', 'I', 'J', 'S', 'Z' -> {
                    return pool[u2()];
                }
                case 'e' -> {
                    u2();
                    return utf(u2());
                }
                case 'c' -> {
                    return utf(u2());
                }
                case '@' -> {
                    return annotation();
                }
                case '[' -> {
                    int n = u2();
                    List<Object> out = new ArrayList<>(n);
                    for (int i = 0; i < n; i++) out.add(value());
                    return out;
                }
                default -> throw new IllegalStateException("annotation value " + tag);
            }
        }
    }

    private static final String[] PLAYER_TICK = {
        "net.minecraft.world.entity.Entity.tick",
        "net.minecraft.world.entity.LivingEntity.tick",
        "net.minecraft.world.entity.player.Player.tick",
        "net.minecraft.server.level.ServerPlayer.tick",
    };

    /**
     * A server tick of `level`'s players: the handlers mods inject into their tick (Entity's,
     * LivingEntity's, Player's and ServerPlayer's, in the order vanilla's calls reach them).
     * Whether there are any, so Horizon keeps asking every tick.
     */
    static boolean tickPlayers(net.minecraft.server.level.ServerLevel level,
            Iterable<net.minecraft.server.level.ServerPlayer> players) {
        boolean any = false;
        for (String target : PLAYER_TICK) any |= !handlers(target).isEmpty();
        if (!any) return false;
        for (var player : players) {
            if (player.level() != level) continue;
            for (String target : PLAYER_TICK) fire(target, player, new Object[0], false);
        }
        return true;
    }

    /** {target, handlers} for the runtime's log and tests: how many handlers each method has. */
    public static Object[] counts(String[] targets) {
        int[] out = new int[targets.length];
        for (int i = 0; i < targets.length; i++) out[i] = handlers(targets[i]).size();
        return new Object[] {out};
    }
}
