package dev.horizonlab.platform120;

/** Objects made without their constructors (sun.misc.Unsafe, in jdk.unsupported). */
public final class Allocate {
    private Allocate() {}

    private static final sun.misc.Unsafe UNSAFE;

    static {
        try {
            var field = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = (sun.misc.Unsafe) field.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    public static <T> T make(Class<T> type) {
        T made;
        try {
            made = type.cast(UNSAFE.allocateInstance(type));
        } catch (InstantiationException e) {
            throw new IllegalStateException(e);
        }
        mixinCollections(made);
        return made;
    }

    /**
     * The collections mods' mixins add to a class, which a skipped constructor leaves null.
     *
     * <p>A mixin's field initializer ({@code Int2ObjectMap<PartEntity<?>> multiparts = new
     * Int2ObjectOpenHashMap<>()}) is merged into every constructor of its target, so an object
     * made without one has the field null, and the first mod to read it throws -- Porting Lib
     * on every block placed, through the level's entity lookup. Constructed120 replays the
     * injections at a constructor's return; this is the other half, the initializers.
     *
     * <p>Only fields a mixin added are touched: their names have a '$' (a mod's prefix, or
     * Mixin's own renaming of a unique field), which no field of Minecraft's has; or, on a
     * Minecraft class, any name but an intermediary one ({@code field_1234}, which every field
     * Minecraft declares has at run time): Fabric API's own mixins name theirs plainly
     * (ServerLevel's {@code loadedChunks}, which its chunk-load listener adds to). Only empty
     * collections are made, of the kind the field's type names -- which is what such an
     * initializer is, nearly always. Anything else stays null.
     */
    static void mixinCollections(Object made) {
        for (Class<?> c = made.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (var field : c.getDeclaredFields()) {
                boolean added = field.getName().indexOf('$') >= 0
                        || (c.getName().startsWith("net.minecraft.") && !field.getName().startsWith("field_"));
                if (java.lang.reflect.Modifier.isStatic(field.getModifiers())
                        || field.getType().isPrimitive()
                        || !added) {
                    continue;
                }
                long offset = UNSAFE.objectFieldOffset(field);
                if (UNSAFE.getObject(made, offset) != null) {
                    continue;
                }
                Object empty = emptyOf(field.getType());
                if (empty != null) {
                    UNSAFE.putObject(made, offset, empty);
                }
            }
        }
    }

    /** An empty collection of the kind `type` names, or null for a type that isn't one. */
    private static Object emptyOf(Class<?> type) {
        String name = type.getName();
        if (type == java.util.Map.class) return new java.util.HashMap<>();
        if (type == java.util.List.class || type == java.util.Collection.class) return new java.util.ArrayList<>();
        if (type == java.util.Set.class) return new java.util.HashSet<>();
        if (type == java.util.Queue.class || type == java.util.Deque.class) return new java.util.ArrayDeque<>();
        if (type == java.util.SortedMap.class || type == java.util.NavigableMap.class) return new java.util.TreeMap<>();
        Class<?> concrete = type;
        if (type.isInterface() && name.startsWith("it.unimi.dsi.fastutil.")) {
            // Int2ObjectMap -> Int2ObjectOpenHashMap; LongSet -> LongOpenHashSet; IntList -> IntArrayList.
            String simple = type.getSimpleName();
            String made = simple.endsWith("Map") ? simple.substring(0, simple.length() - 3) + "OpenHashMap"
                    : simple.endsWith("Set") ? simple.substring(0, simple.length() - 3) + "OpenHashSet"
                    : simple.endsWith("List") ? simple.substring(0, simple.length() - 4) + "ArrayList"
                    : null;
            if (made == null) return null;
            try {
                concrete = Class.forName(type.getPackageName() + "." + made, true, type.getClassLoader());
            } catch (ClassNotFoundException e) {
                return null;
            }
        }
        if (concrete.isInterface() || java.lang.reflect.Modifier.isAbstract(concrete.getModifiers())
                || !(java.util.Map.class.isAssignableFrom(concrete) || java.util.Collection.class.isAssignableFrom(concrete))) {
            return null;
        }
        try {
            return concrete.getConstructor().newInstance();
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }
}
