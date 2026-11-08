package dev.horizonlab.bridge.r120;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.LinkedHashMap;
import java.util.stream.Stream;

/**
 * What a 1.20.1 boot holds, as JSON (phase 1's milestone, like Report112): the mods (their
 * versions, declared dependencies and Mixin configs, how their entrypoints went), every block
 * with its properties and state count, items, entity types, fluids, block entity types, menus,
 * biomes and dimension types (vanilla's from the code, the mods' from their data files), the
 * timings. Each section fails on its own: a broken one says why and the rest still report.
 */
final class Report120 {
    private final Boot120 boot;
    private final Names120 n;

    Report120(Boot120 boot) {
        this.boot = boot;
        this.n = boot.names();
    }

    String write() {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("minecraft", "1.20.1");
        out.put("java", Runtime.version().toString());
        out.put("side", boot.options().side);
        out.putAll(boot.timings());
        out.put("mapped_classes", n == null ? 0 : n.classes());
        section(out, "mods", this::mods);
        out.put("entrypoints", boot.entrypoints());
        section(out, "blocks", this::blocks);
        section(out, "items", this::items);
        section(out, "entity_types", () -> keys("ENTITY_TYPE", this::entityType));
        section(out, "fluids", () -> keys("FLUID", null));
        section(out, "block_entity_types", () -> keys("BLOCK_ENTITY_TYPE", null));
        section(out, "menus", () -> keys("MENU", null));
        section(out, "biomes", () -> vanillaAndData("BIOME", "worldgen/biome"));
        section(out, "dimension_types", () -> vanillaAndData("DIMENSION_TYPE", "dimension_type"));
        section(out, "dimensions", () -> vanillaAndData(null, "dimension"));
        out.put("problems", boot.problems());
        return Json.write(out);
    }

    private interface Part {
        Object get() throws Exception;
    }

    private void section(Map<String, Object> out, String name, Part part) {
        try {
            out.put(name, part.get());
        } catch (Throwable e) {
            out.put(name, Map.of("error", Boot120.root(e)));
            boot.problems().add(name + ": " + Boot120.root(e));
        }
    }

    private static Object invoke(Object target, String method) throws ReflectiveOperationException {
        return Api.call(target, method);
    }

    private List<?> allMods() throws ReflectiveOperationException {
        Class<?> api = Class.forName("net.fabricmc.loader.api.FabricLoader");
        Object fabric = api.getMethod("getInstance").invoke(null);
        return new ArrayList<>((Collection<?>) api.getMethod("getAllMods").invoke(fabric));
    }

    @SuppressWarnings("unchecked")
    private Object mods() throws ReflectiveOperationException {
        Map<String, Object> out = new TreeMap<>();
        Object env = Enum.valueOf(envType(), boot.options().side.toUpperCase(java.util.Locale.ROOT));
        for (Object mod : allMods()) {
            Object meta = invoke(mod, "getMetadata");
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("version", invoke(invoke(meta, "getVersion"), "getFriendlyString"));
            m.put("name", invoke(meta, "getName"));
            Map<String, String> deps = new TreeMap<>();
            for (Object dep : (Collection<?>) invoke(meta, "getDependencies")) {
                String kind = String.valueOf(invoke(dep, "getKind")).toLowerCase(java.util.Locale.ROOT);
                String target = (String) invoke(dep, "getModId");
                deps.put(kind + " " + target, String.valueOf(invoke(dep, "getVersionRequirements")));
            }
            if (!deps.isEmpty()) m.put("dependencies", deps);
            try {
                // LoaderModMetadata (Fabric's own type): the Mixin configs this side applies.
                Collection<?> mixins = (Collection<?>) Api.call(meta, "getMixinConfigs", env);
                if (!mixins.isEmpty()) m.put("mixins", new ArrayList<>(mixins));
            } catch (ReflectiveOperationException e) {
                // Not Fabric's metadata (a built-in mod).
            }
            m.put("nested", ((java.util.Optional<?>) invoke(mod, "getContainingMod")).isPresent());
            out.put((String) invoke(meta, "getId"), m);
        }
        return out;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Class<Enum> envType() throws ClassNotFoundException {
        return (Class<Enum>) Class.forName("net.fabricmc.api.EnvType");
    }

    private Object registry(String field) throws ReflectiveOperationException {
        return n.getStatic(n.cls("net.minecraft.core.registries.BuiltInRegistries", boot.game()), field);
    }

    private String key(Object registry, Object entry) throws ReflectiveOperationException {
        return String.valueOf(n.call(registry, "getKey", entry));
    }

    private interface Describe {
        void fill(Object entry, Map<String, Object> into) throws ReflectiveOperationException;
    }

    /** A registry's ids, sorted; with {@code describe}, each with its facts. */
    private Object keys(String field, Describe describe) throws ReflectiveOperationException {
        Object registry = registry(field);
        Map<String, Object> out = new TreeMap<>();
        for (Object entry : (Iterable<?>) registry) {
            Map<String, Object> facts = new LinkedHashMap<>();
            if (describe != null) describe.fill(entry, facts);
            out.put(key(registry, entry), facts);
        }
        if (describe == null) return new ArrayList<>(out.keySet());
        return out;
    }

    private Object blocks() throws ReflectiveOperationException {
        return keys("BLOCK", (block, facts) -> {
            Object definition = n.call(block, "getStateDefinition");
            facts.put("states", ((Collection<?>) n.call(definition, "getPossibleStates")).size());
            Map<String, Object> properties = new LinkedHashMap<>();
            for (Object property : (Collection<?>) n.call(definition, "getProperties")) {
                List<String> values = new ArrayList<>();
                for (Object value : (Collection<?>) n.call(property, "getPossibleValues")) {
                    values.add(String.valueOf(n.call(property, "getName", value)));
                }
                properties.put((String) n.call(property, "getName"), values);
            }
            if (!properties.isEmpty()) facts.put("properties", properties);
            Object state = n.call(block, "defaultBlockState");
            int light = (Integer) n.call(state, "getLightEmission");
            if (light > 0) facts.put("light", light);
            facts.put("class", className(block));
        });
    }

    private Object items() throws ReflectiveOperationException {
        return keys("ITEM", (item, facts) -> {
            facts.put("stack", n.call(item, "getMaxStackSize"));
            int damage = (Integer) n.call(item, "getMaxDamage");
            if (damage > 0) facts.put("damage", damage);
            facts.put("class", className(item));
        });
    }

    private void entityType(Object type, Map<String, Object> facts) throws ReflectiveOperationException {
        facts.put("category", n.call(n.call(type, "getCategory"), "getName"));
    }

    /** A game class by Mojang's name; a mod's class as it is. */
    private String className(Object o) {
        String runtime = o.getClass().getName();
        String mojang = n.mojangClass(runtime);
        return mojang != null ? mojang : runtime;
    }

    /**
     * Vanilla's entries of a data-driven registry, from the code that makes them
     * ({@code VanillaRegistries.createLookup()}, as the data generator does), and the mods' own
     * data files under {@code data/<ns>/<folder>/}. Mods that add entries through code at a
     * server's start (Fabric API's biome API, TerraBlender) show up only when a server loads:
     * a later phase.
     */
    private Object vanillaAndData(String registriesField, String folder) throws Exception {
        Map<String, Object> out = new LinkedHashMap<>();
        if (registriesField != null) {
            Object lookup = n.callStatic(n.cls("net.minecraft.data.registries.VanillaRegistries", boot.game()), "createLookup");
            Object key = n.getStatic(n.cls("net.minecraft.core.registries.Registries", boot.game()), registriesField);
            Object registry = n.call(lookup, "lookupOrThrow", key);
            List<String> vanilla = new ArrayList<>();
            try (Stream<?> ids = (Stream<?>) n.call(registry, "listElementIds")) {
                for (Object id : ids.toArray()) vanilla.add(String.valueOf(n.call(id, "location")));
            }
            vanilla.sort(null);
            out.put("vanilla", vanilla);
        }
        Map<String, List<String>> fromMods = new TreeMap<>();
        for (Object mod : allMods()) {
            String id = (String) invoke(invoke(mod, "getMetadata"), "getId");
            if (id.equals("minecraft") || id.equals("java")) continue;
            List<String> found = new ArrayList<>();
            for (Object rootObj : (List<?>) invoke(mod, "getRootPaths")) {
                Path data = ((Path) rootObj).resolve("data");
                if (!Files.isDirectory(data)) continue;
                try (Stream<Path> namespaces = Files.list(data)) {
                    for (Path ns : (Iterable<Path>) namespaces::iterator) {
                        Path dir = ns.resolve(folder);
                        if (!Files.isDirectory(dir)) continue;
                        try (Stream<Path> files = Files.walk(dir)) {
                            for (Path f : (Iterable<Path>) files::iterator) {
                                String rel = dir.relativize(f).toString().replace('\\', '/');
                                if (rel.endsWith(".json")) {
                                    found.add(ns.getFileName().toString().replace("/", "") + ":" + rel.substring(0, rel.length() - 5));
                                }
                            }
                        }
                    }
                }
            }
            if (!found.isEmpty()) {
                found.sort(null);
                fromMods.put(id, found);
            }
        }
        out.put("mods", fromMods);
        return out;
    }
}
