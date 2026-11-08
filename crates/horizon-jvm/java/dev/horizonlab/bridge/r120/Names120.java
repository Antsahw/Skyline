package dev.horizonlab.bridge.r120;

import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Mojang's names for 1.20.1's classes and members, at run time. 1.20.1 is obfuscated and Fabric
 * runs it under intermediary names ({@code net.minecraft.class_2248}), so the bridge's 1.20.1
 * code, written with Mojang's names like the 26.3 platform, looks its targets up here:
 * Mojang's mappings ({@code client.txt}: Mojang's name to the official, obfuscated one), then
 * official to whatever the game runs under, through Fabric Loader's own mapping resolver
 * ({@link Translator}).
 *
 * <p>Members are found by name and argument count, walking the class's supertypes, as the
 * mappings list a method only under the class that declares it. A name the mappings don't
 * know (a JDK method: {@code iterator}, {@code toString}) is looked up as it is.
 */
public final class Names120 {
    /** Official names to the run-time namespace (Fabric's MappingResolver, or a test's map). */
    public interface Translator {
        String cls(String official);

        String method(String officialOwner, String name, String officialDescriptor);

        String field(String officialOwner, String name, String officialDescriptor);
    }

    private record Member(String name, String official, String descriptor, int arity) {}

    private static final class Entry {
        final String official;
        final List<Member> methods = new ArrayList<>();
        final List<Member> fields = new ArrayList<>();

        Entry(String official) {
            this.official = official;
        }
    }

    private final Map<String, Entry> byMojang = new HashMap<>();
    private final Map<String, String> mojangByRuntime = new HashMap<>();
    private final Map<String, String> runtimeByMojang = new HashMap<>();
    private final Map<String, String> officialByMojang = new HashMap<>();
    private final Translator translator;

    private Names120(Translator translator) {
        this.translator = translator;
    }

    /** Reads Mojang's mappings file. */
    public static Names120 load(Path mojangMappings, Translator translator) throws IOException {
        Names120 names = new Names120(translator);
        // Two passes over the lines: descriptors need every class's official name first.
        List<String> lines = Files.readAllLines(mojangMappings, StandardCharsets.UTF_8);
        for (String line : lines) {
            if (line.isEmpty() || line.charAt(0) == '#' || line.charAt(0) == ' ') continue;
            int arrow = line.indexOf(" -> ");
            if (arrow < 0 || !line.endsWith(":")) continue;
            names.officialByMojang.put(line.substring(0, arrow), line.substring(arrow + 4, line.length() - 1));
        }
        Entry current = null;
        for (String line : lines) {
            if (line.isEmpty() || line.charAt(0) == '#') continue;
            if (line.charAt(0) != ' ') {
                int arrow = line.indexOf(" -> ");
                current = null;
                if (arrow < 0) continue;
                String mojang = line.substring(0, arrow);
                current = new Entry(names.officialByMojang.get(mojang));
                names.byMojang.put(mojang, current);
                continue;
            }
            if (current == null) continue;
            names.member(current, line.trim());
        }
        for (Map.Entry<String, Entry> e : names.byMojang.entrySet()) {
            String runtime = translator.cls(e.getValue().official);
            names.runtimeByMojang.put(e.getKey(), runtime);
            names.mojangByRuntime.put(runtime, e.getKey());
        }
        return names;
    }

    private void member(Entry owner, String line) {
        int arrow = line.lastIndexOf(" -> ");
        if (arrow < 0) return;
        String official = line.substring(arrow + 4);
        String left = line.substring(0, arrow);
        // "12:15:void name(args)" (method with lines), "void name(args)", "Type field".
        int paren = left.indexOf('(');
        if (paren >= 0) {
            String head = left.substring(0, paren);
            while (!head.isEmpty() && Character.isDigit(head.charAt(0))) {
                int colon = head.indexOf(':');
                if (colon < 0) break;
                head = head.substring(colon + 1);
            }
            int space = head.lastIndexOf(' ');
            String returnType = head.substring(0, space);
            String name = head.substring(space + 1);
            String argText = left.substring(paren + 1, left.indexOf(')', paren));
            String[] args = argText.isEmpty() ? new String[0] : argText.split(",");
            StringBuilder desc = new StringBuilder("(");
            for (String a : args) desc.append(descriptor(a));
            desc.append(')').append(descriptor(returnType));
            owner.methods.add(new Member(name, official, desc.toString(), args.length));
        } else {
            int space = left.lastIndexOf(' ');
            owner.fields.add(new Member(left.substring(space + 1), official, descriptor(left.substring(0, space)), -1));
        }
    }

    /** A Mojang-named Java type ({@code int}, {@code net.minecraft.core.BlockPos[]}) as an official descriptor. */
    String descriptor(String type) {
        int dims = 0;
        while (type.endsWith("[]")) {
            dims++;
            type = type.substring(0, type.length() - 2);
        }
        String d = switch (type) {
            case "void" -> "V";
            case "boolean" -> "Z";
            case "byte" -> "B";
            case "char" -> "C";
            case "short" -> "S";
            case "int" -> "I";
            case "long" -> "J";
            case "float" -> "F";
            case "double" -> "D";
            default -> "L" + officialByMojang.getOrDefault(type, type).replace('.', '/') + ";";
        };
        return "[".repeat(dims) + d;
    }

    /** The run-time name of a class Mojang calls {@code mojang} (unknown classes: as they are). */
    public String runtimeClass(String mojang) {
        return runtimeByMojang.getOrDefault(mojang, mojang);
    }

    /** Mojang's name of a run-time class name, or {@code null} for a class not in the mappings. */
    public String mojangClass(String runtime) {
        return mojangByRuntime.get(runtime);
    }

    /** A game class by Mojang's name, loaded (and initialized) by {@code loader}. */
    public Class<?> cls(String mojang, ClassLoader loader) throws ClassNotFoundException {
        return Class.forName(runtimeClass(mojang), true, loader);
    }

    /** Run-time names a Mojang-named method may have in {@code type} (by argument count). */
    private Set<String> methodNames(Class<?> type, String name, int arity) {
        Set<String> out = new HashSet<>();
        String mojang = mojangClass(type.getName());
        Entry entry = mojang == null ? null : byMojang.get(mojang);
        if (entry != null) {
            for (Member m : entry.methods) {
                if (m.name.equals(name) && m.arity == arity) out.add(translator.method(entry.official, m.official, m.descriptor));
            }
        }
        return out;
    }

    /** Every supertype, nearest first: the class, its superclasses and all interfaces. */
    private static List<Class<?>> hierarchy(Class<?> type) {
        List<Class<?>> out = new ArrayList<>();
        ArrayDeque<Class<?>> queue = new ArrayDeque<>();
        Set<Class<?>> seen = new HashSet<>();
        queue.add(type);
        while (!queue.isEmpty()) {
            Class<?> c = queue.poll();
            if (c == null || !seen.add(c)) continue;
            out.add(c);
            if (c.getSuperclass() != null) queue.add(c.getSuperclass());
            for (Class<?> i : c.getInterfaces()) queue.add(i);
        }
        return out;
    }

    /**
     * The method Mojang names {@code name} on {@code type} or a supertype that takes {@code args}
     * (argument count first, then the argument types decide between overloads).
     */
    public Method method(Class<?> type, String name, Object... args) throws NoSuchMethodException {
        for (Class<?> c : hierarchy(type)) {
            Set<String> wanted = methodNames(c, name, args.length);
            for (Method m : c.getDeclaredMethods()) {
                if (m.getParameterCount() != args.length || m.isSynthetic() && !m.isBridge()) continue;
                boolean named = wanted.contains(m.getName()) || wanted.isEmpty() && m.getName().equals(name);
                if (named && applicable(m.getParameterTypes(), args)) {
                    m.setAccessible(true);
                    return m;
                }
            }
        }
        throw new NoSuchMethodException(type.getName() + " (" + mojangClass(type.getName()) + ")." + name + " with " + args.length + " arguments");
    }

    private static boolean applicable(Class<?>[] params, Object[] args) {
        for (int i = 0; i < params.length; i++) {
            Class<?> p = params[i];
            Object a = args[i];
            if (a == null) {
                if (p.isPrimitive()) return false;
                continue;
            }
            if (p.isPrimitive()) p = box(p);
            if (!p.isInstance(a)) return false;
        }
        return true;
    }

    private static Class<?> box(Class<?> p) {
        if (p == int.class) return Integer.class;
        if (p == long.class) return Long.class;
        if (p == boolean.class) return Boolean.class;
        if (p == float.class) return Float.class;
        if (p == double.class) return Double.class;
        if (p == byte.class) return Byte.class;
        if (p == short.class) return Short.class;
        if (p == char.class) return Character.class;
        return Void.class;
    }

    /** The field Mojang names {@code name} on {@code type} or a superclass. */
    public Field field(Class<?> type, String name) throws NoSuchFieldException {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            String mojang = mojangClass(c.getName());
            Entry entry = mojang == null ? null : byMojang.get(mojang);
            String runtime = name;
            if (entry != null) {
                for (Member f : entry.fields) {
                    if (f.name.equals(name)) runtime = translator.field(entry.official, f.official, f.descriptor);
                }
            }
            try {
                Field f = c.getDeclaredField(runtime);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException e) {
                // A superclass may declare it.
            }
        }
        throw new NoSuchFieldException(type.getName() + " (" + mojangClass(type.getName()) + ")." + name);
    }

    /** Calls the method Mojang names {@code name} on {@code target}. */
    public Object call(Object target, String name, Object... args) throws ReflectiveOperationException {
        return method(target.getClass(), name, args).invoke(target, args);
    }

    /** Calls a static method of {@code type}. */
    public Object callStatic(Class<?> type, String name, Object... args) throws ReflectiveOperationException {
        Method m = method(type, name, args);
        if (!Modifier.isStatic(m.getModifiers())) throw new NoSuchMethodException(name + " is not static");
        return m.invoke(null, args);
    }

    /** A static field's value. */
    public Object getStatic(Class<?> type, String name) throws ReflectiveOperationException {
        return field(type, name).get(null);
    }

    /** How many classes the mappings name (for the report). */
    public int classes() {
        return byMojang.size();
    }
}
