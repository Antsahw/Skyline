package dev.horizonlab.bridge.r120;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayDeque;
import java.util.HashSet;
import java.util.Set;

/**
 * Calls Fabric Loader's API by reflection (this code compiles with JDK types only). Fabric's
 * implementations are often package-private classes, so methods are looked up on a public
 * type the object has: its public interfaces ({@code ModContainer}, {@code ModMetadata}...)
 * or public superclasses.
 */
final class Api {
    private Api() {}

    static Object call(Object target, String name, Object... args) throws ReflectiveOperationException {
        return method(target.getClass(), name, args.length).invoke(target, args);
    }

    static Method method(Class<?> type, String name, int arity) throws NoSuchMethodException {
        ArrayDeque<Class<?>> queue = new ArrayDeque<>();
        Set<Class<?>> seen = new HashSet<>();
        queue.add(type);
        while (!queue.isEmpty()) {
            Class<?> c = queue.poll();
            if (!seen.add(c)) continue;
            if (Modifier.isPublic(c.getModifiers())) {
                for (Method m : c.getMethods()) {
                    if (m.getName().equals(name) && m.getParameterCount() == arity && Modifier.isPublic(m.getDeclaringClass().getModifiers())) return m;
                }
            }
            if (c.getSuperclass() != null) queue.add(c.getSuperclass());
            for (Class<?> i : c.getInterfaces()) queue.add(i);
        }
        throw new NoSuchMethodException(type.getName() + "." + name + " with " + arity + " arguments on a public type");
    }
}
