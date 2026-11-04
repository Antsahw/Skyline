package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Mods' client state kept per player. One headless client runs every player's client in turn
 * (HorizonClient.as), so what a mod keeps in a static field or a singleton is shared by them
 * all: the host typed "iron" in a mod's search box and the guest's showed it. The fields listed
 * here are swapped as the client changes player: each player's values are put away when another
 * player's client runs, and come back with theirs. A player new to the client starts from the
 * field's empty value (null, false, 0), which these mods fill in lazily .
 */
final class HorizonClientState {
    private HorizonClientState() {}

    /**
     * A field of client state: static, or of the instance {@code owner} returns (a method) or
     * holds (a static field); {@code fresh} makes a new player's value (null: the field's
     * empty value).
     */
    private record Slot(Field field, java.lang.reflect.Member owner, java.util.concurrent.Callable<Object> fresh) {
        Object target() throws ReflectiveOperationException {
            if (owner instanceof Method m) return m.invoke(null);
            if (owner instanceof Field f) return f.get(null);
            return null;
        }

        Object first() throws Exception {
            return fresh != null ? fresh.call() : empty(field.getType());
        }
    }

    /**
     * {@code Class(arg)}: a new instance by its constructor, with no argument or one enum
     * constant ({@code pkg.Enum.CONSTANT}).
     */
    private static java.util.concurrent.Callable<Object> factory(String spec, ClassLoader loader) throws ReflectiveOperationException {
        int open = spec.indexOf('(');
        Class<?> type = Class.forName(spec.substring(0, open), false, loader);
        String arg = spec.substring(open + 1, spec.length() - 1);
        if (arg.isEmpty()) {
            var make = type.getDeclaredConstructor();
            make.setAccessible(true);
            return make::newInstance;
        }
        int dot = arg.lastIndexOf('.');
        @SuppressWarnings({"unchecked", "rawtypes"})
        Object constant = Enum.valueOf((Class) Class.forName(arg.substring(0, dot), true, loader), arg.substring(dot + 1));
        for (var make : type.getDeclaredConstructors()) {
            if (make.getParameterCount() == 1 && make.getParameterTypes()[0].isInstance(constant)) {
                make.setAccessible(true);
                return () -> make.newInstance(constant);
            }
        }
        throw new NoSuchMethodException(spec);
    }

    /**
     * The fields, as {@code class#field} (static) or {@code class#getInstance()#field} (of the
     * instance a static method returns). A class that isn't there (its mod isn't) is skipped.
     */
    private static final String[] FIELDS = {
    };

    private static List<Slot> slots;
    /** Each player's put-away values, by slot. */
    private static final Map<UUID, Object[]> SAVED = new ConcurrentHashMap<>();
    /** The player whose values are in the fields now (null: nobody's yet). */
    private static UUID loaded;

    /** The player whose values are in the fields now (null: nobody's yet). */
    static UUID loaded() {
        return loaded;
    }

    private static synchronized List<Slot> slots() {
        if (slots != null) return slots;
        List<Slot> made = new ArrayList<>();
        ClassLoader loader = HorizonClientState.class.getClassLoader();
        for (String full : FIELDS) {
            String[] made2 = full.split("=", 2);
            String spec = made2[0];
            String[] parts = spec.split("#");
            try {
                Class<?> owner = Class.forName(parts[0], false, loader);
                java.lang.reflect.Member instance = null;
                Class<?> holder = owner;
                if (parts.length == 3) {
                    if (parts[1].endsWith("()")) {
                        Method m = owner.getMethod(parts[1].replace("()", ""));
                        instance = m;
                        holder = m.getReturnType();
                    } else {
                        Field f = owner.getDeclaredField(parts[1]);
                        f.setAccessible(true);
                        instance = f;
                        holder = f.getType();
                    }
                }
                Field field = holder.getDeclaredField(parts[parts.length - 1]);
                if (Modifier.isFinal(field.getModifiers())) {
                    Natives.log("Horizon's client: per-player state " + spec + " is final; kept for all players");
                    continue;
                }
                field.setAccessible(true);
                made.add(new Slot(field, instance, made2.length == 2 ? factory(made2[1], loader) : null));
            } catch (ClassNotFoundException e) {
                // The mod isn't here.
            } catch (ReflectiveOperationException | RuntimeException e) {
                Natives.log("Horizon's client: per-player state " + spec + " can't be kept: " + e);
            }
        }
        slots = made;
        return made;
    }

    /** The fields hold {@code player}'s values (put away whose were there). */
    static synchronized void load(UUID player) {
        if (player == null || player.equals(loaded)) return;
        // Their mods' settings too (HorizonClientConfig).
        HorizonClientConfig.swap(loaded, player);
        List<Slot> all = slots();
        if (all.isEmpty()) {
            loaded = player;
            return;
        }
        try {
            if (loaded != null) {
                Object[] values = new Object[all.size()];
                for (int i = 0; i < all.size(); i++) {
                    Slot slot = all.get(i);
                    values[i] = slot.field().get(slot.target());
                }
                SAVED.put(loaded, values);
            }
            Object[] values = SAVED.get(player);
            for (int i = 0; i < all.size(); i++) {
                Slot slot = all.get(i);
                Object value = values != null ? values[i] : null;
                if (values == null) {
                    // The first player keeps what the mod made (its own objects, started).
                    Object current = loaded == null ? slot.field().get(slot.target()) : null;
                    value = current != null ? current : slot.first();
                }
                slot.field().set(slot.target(), value);
            }
            loaded = player;
        } catch (Exception e) {
            Throwable root = e;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's client: per-player state couldn't be swapped: " + root);
        }
    }

    /** A player left: their values go (and the fields', when theirs are in them). */
    static synchronized void forget(UUID player) {
        SAVED.remove(player);
        HorizonClientConfig.forget(player);
        List<Slot> all = slots();
        if (player.equals(loaded)) {
            for (Slot slot : all) {
                try {
                    slot.field().set(slot.target(), slot.first());
                } catch (Exception e) {
                    // Left as it is.
                }
            }
            loaded = null;
        }
    }

    private static Object empty(Class<?> type) {
        if (!type.isPrimitive()) {
            // A collection a mod makes once and never again (REI's lastDisplayScreen, cleared at
            // each reload): a new empty one of its class.
            if ((java.util.Collection.class.isAssignableFrom(type) || java.util.Map.class.isAssignableFrom(type))
                    && !type.isInterface() && !Modifier.isAbstract(type.getModifiers())) {
                try {
                    return type.getDeclaredConstructor().newInstance();
                } catch (ReflectiveOperationException e) {
                    return null;
                }
            }
            return null;
        }
        if (type == boolean.class) return false;
        if (type == char.class) return (char) 0;
        if (type == long.class) return 0L;
        if (type == float.class) return 0F;
        if (type == double.class) return 0D;
        if (type == byte.class) return (byte) 0;
        if (type == short.class) return (short) 0;
        return 0;
    }
}
