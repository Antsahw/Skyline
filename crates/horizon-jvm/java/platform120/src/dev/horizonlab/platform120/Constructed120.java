package dev.horizonlab.platform120;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.AnnotationNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.transformer.meta.MixinMerged;

/**
 * What mods add at the end of a constructor, for an object the mirror makes without it
 * (Mirror120's server and levels): Lithium caches a level's height there, other mods set up
 * per-level state. For each class from the top down, the mixins merged into it (their handlers'
 * {@code @MixinMerged}) are read from their mods' jars,
 * and their {@code @Inject(method = "<init>", at = @At("RETURN" | "TAIL"))} handlers, merged into
 * the class, are called. A handler's parameters are the constructor's: each gets a value of its
 * type from {@code arguments} (what the constructor would have been given), else from the
 * object's own fields, else null (zero). A failing handler is logged and the rest go on.
 * (The 26.3 bridge's HorizonHooks.constructed, smaller: no field initializers, no wrapped calls.)
 */
final class Constructed120 {
    private Constructed120() {}

    static void run(Object self, Object... arguments) {
        List<Class<?>> chain = new ArrayList<>();
        for (Class<?> c = self.getClass(); c != null && c != Object.class; c = c.getSuperclass()) chain.add(0, c);
        for (Class<?> owner : chain) {
            // The mixins merged into the class, from their handlers' @MixinMerged.
            java.util.Set<String> mixins = new java.util.LinkedHashSet<>();
            for (Method m : owner.getDeclaredMethods()) {
                MixinMerged merged = m.getAnnotation(MixinMerged.class);
                if (merged != null) mixins.add(merged.mixin());
            }
            for (String mixin : mixins) {
                ClassNode node = read(owner.getClassLoader(), mixin);
                if (node == null) continue;
                for (MethodNode m : node.methods) {
                    if (!endOfConstructor(m)) continue;
                    Method handler = merged(owner, m, mixin);
                    if (handler == null) continue;
                    call(owner, self, handler, arguments, mixin);
                }
            }
        }
    }

    /** A mixin's class as its mod's jar has it (annotations only). */
    private static ClassNode read(ClassLoader loader, String mixin) {
        try (var in = loader.getResourceAsStream(mixin.replace('.', '/') + ".class")) {
            if (in == null) return null;
            ClassNode node = new ClassNode();
            new org.objectweb.asm.ClassReader(in.readAllBytes()).accept(node,
                    org.objectweb.asm.ClassReader.SKIP_CODE | org.objectweb.asm.ClassReader.SKIP_DEBUG | org.objectweb.asm.ClassReader.SKIP_FRAMES);
            return node;
        } catch (java.io.IOException | RuntimeException e) {
            return null;
        }
    }

    /** An @Inject into a constructor at its end (RETURN or TAIL). */
    private static boolean endOfConstructor(MethodNode m) {
        for (AnnotationNode a : annotations(m)) {
            if (!"Lorg/spongepowered/asm/mixin/injection/Inject;".equals(a.desc)) continue;
            Object methods = get(a, "method");
            boolean constructor = methods instanceof List<?> l && l.stream().anyMatch(s -> String.valueOf(s).startsWith("<init>"));
            if (!constructor) continue;
            Object at = get(a, "at");
            List<?> ats = at instanceof List<?> l ? l : List.of(at);
            for (Object one : ats) {
                if (one instanceof AnnotationNode point) {
                    Object value = get(point, "value");
                    if ("RETURN".equals(value) || "TAIL".equals(value)) return true;
                }
            }
        }
        return false;
    }

    private static List<AnnotationNode> annotations(MethodNode m) {
        List<AnnotationNode> all = new ArrayList<>();
        if (m.visibleAnnotations != null) all.addAll(m.visibleAnnotations);
        if (m.invisibleAnnotations != null) all.addAll(m.invisibleAnnotations);
        return all;
    }

    private static Object get(AnnotationNode a, String key) {
        if (a.values == null) return null;
        for (int i = 0; i + 1 < a.values.size(); i += 2) {
            if (key.equals(a.values.get(i))) {
                Object v = a.values.get(i + 1);
                // An enum value: [descriptor, name].
                if (v instanceof String[] e && e.length == 2) return e[1];
                return v;
            }
        }
        return null;
    }

    /** The handler as merged into the target: `handler$<id>$<mod>$<name>`, same descriptor. */
    private static Method merged(Class<?> owner, MethodNode m, String mixin) {
        for (Method candidate : owner.getDeclaredMethods()) {
            MixinMerged merged = candidate.getAnnotation(MixinMerged.class);
            if (merged == null || !merged.mixin().equals(mixin)) continue;
            String name = candidate.getName();
            if (!(name.equals(m.name) || name.endsWith("$" + m.name))) continue;
            if (!Type.getMethodDescriptor(candidate).equals(m.desc)) continue;
            return candidate;
        }
        return null;
    }

    private static void call(Class<?> owner, Object self, Method handler, Object[] arguments, String mixin) {
        Class<?>[] params = handler.getParameterTypes();
        Object[] values = new Object[params.length];
        for (int i = 0; i < params.length; i++) {
            Class<?> type = params[i];
            if (CallbackInfo.class.isAssignableFrom(type)) {
                values[i] = new CallbackInfo("<init>", false);
                continue;
            }
            Object value = of(type, arguments);
            if (value == null) value = field(self, type);
            if (value == null && type.isPrimitive()) value = zero(type);
            values[i] = value;
        }
        try {
            handler.setAccessible(true);
            handler.invoke(Modifier.isStatic(handler.getModifiers()) ? null : self, values);
        } catch (Throwable t) {
            Throwable cause = t instanceof java.lang.reflect.InvocationTargetException ite && ite.getCause() != null ? ite.getCause() : t;
            Mirror120.log("a mod failed at the end of " + owner.getSimpleName() + "'s constructor (" + mixin + "): " + cause);
        }
    }

    private static Object of(Class<?> type, Object[] values) {
        for (Object v : values) {
            if (v == null) continue;
            if (type.isInstance(v) || (type.isPrimitive() && box(type).isInstance(v))) return v;
        }
        return null;
    }

    private static Object field(Object self, Class<?> type) {
        if (type.isPrimitive() || type == Object.class) return null;
        for (Class<?> c = self.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers()) || !type.isAssignableFrom(f.getType())) continue;
                try {
                    f.setAccessible(true);
                    Object v = f.get(self);
                    if (v != null) return v;
                } catch (ReflectiveOperationException | RuntimeException e) {
                    // not reachable: try the next
                }
            }
        }
        return null;
    }

    private static Class<?> box(Class<?> type) {
        if (type == int.class) return Integer.class;
        if (type == long.class) return Long.class;
        if (type == boolean.class) return Boolean.class;
        if (type == double.class) return Double.class;
        if (type == float.class) return Float.class;
        if (type == short.class) return Short.class;
        if (type == byte.class) return Byte.class;
        if (type == char.class) return Character.class;
        return type;
    }

    private static Object zero(Class<?> type) {
        if (type == boolean.class) return false;
        if (type == long.class) return 0L;
        if (type == double.class) return 0.0;
        if (type == float.class) return 0.0f;
        if (type == char.class) return (char) 0;
        if (type == byte.class) return (byte) 0;
        if (type == short.class) return (short) 0;
        return 0;
    }
}
