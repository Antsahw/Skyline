package dev.horizonlab.bridge.r120;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Mods held back only by their declared Fabric Loader range (docs/java-bridge-120.md, "Mods that
 * need an older Fabric Loader", reason 1). Before Fabric Loader resolves the mods, every mod's
 * {@code fabric.mod.json} (nested jars too) is read; a mod whose {@code fabricloader} range
 * excludes the running loader gets an entry in Fabric Loader's own dependency override file
 * ({@code config/fabric_loader_dependencies.json}): its {@code fabricloader} dependency is
 * dropped, nothing else of it changes, its jar stays as it is.
 *
 * <p>The ranges are tested with Fabric's own public {@code VersionPredicate}, so they mean what
 * they mean to Fabric. The player's own overrides ({@code horizon-dependency-overrides.json} in
 * the game folder, Fabric's format) win over these: an entry there for a mod replaces the
 * automatic one, and {@code {"modid": {}}} keeps a mod's range as declared. The bridge owns
 * the override file in its 1.20.1 game folder and writes it on every start; one it finds that
 * it didn't write is kept as the player's file first.
 */
final class Overrides120 {
    private Overrides120() {}

    static final String FILE = "config/fabric_loader_dependencies.json";
    static final String PLAYER_FILE = "horizon-dependency-overrides.json";
    private static final String STAMP = "config/.horizon-dependency-overrides";

    /** What was overridden: mod id -> {"fabricloader": declared range, "by": "auto"|"player"}. */
    static Map<String, Map<String, String>> write(Path gameDir, Path modsDir, List<Path> addMods) throws Exception {
        Path file = gameDir.resolve(FILE);
        Path player = gameDir.resolve(PLAYER_FILE);
        Path stamp = gameDir.resolve(STAMP);
        Files.createDirectories(file.getParent());
        // A file the bridge didn't write (the player's, from before): it becomes the player's.
        if (Files.exists(file) && !Files.exists(player)) {
            String text = Files.readString(file);
            boolean ours = Files.exists(stamp) && Files.readString(stamp).equals(Integer.toHexString(text.hashCode()));
            if (!ours) Files.writeString(player, text);
        }
        Map<String, Object> playerOverrides = new LinkedHashMap<>();
        if (Files.exists(player)) {
            Object parsed = MiniJson.parse(Files.readString(player));
            if (parsed instanceof Map<?, ?> m && m.get("overrides") instanceof Map<?, ?> o) {
                for (Map.Entry<?, ?> e : o.entrySet()) playerOverrides.put(String.valueOf(e.getKey()), e.getValue());
            }
        }

        String loaderVersion = (String) Class.forName("net.fabricmc.loader.impl.FabricLoaderImpl").getField("VERSION").get(null);
        Class<?> versionClass = Class.forName("net.fabricmc.loader.api.Version");
        Object running = versionClass.getMethod("parse", String.class).invoke(null, loaderVersion);
        Class<?> predicateClass = Class.forName("net.fabricmc.loader.api.metadata.version.VersionPredicate");
        Method parsePredicate = predicateClass.getMethod("parse", String.class);
        Method test = predicateClass.getMethod("test", Object.class); // Predicate<Version>.test, erased

        Map<String, Object> ranges = new TreeMap<>(); // mod id -> declared fabricloader range
        List<Path> jars = new ArrayList<>(addMods);
        if (Files.isDirectory(modsDir)) {
            try (DirectoryStream<Path> d = Files.newDirectoryStream(modsDir, "*.jar")) {
                for (Path p : d) jars.add(p);
            }
        }
        for (Path jar : jars) {
            try (InputStream in = Files.newInputStream(jar)) {
                readJar(in, ranges);
            } catch (IOException | RuntimeException e) {
                // Not a jar, or a damaged fabric.mod.json: Fabric Loader will say so itself.
            }
        }

        Map<String, Map<String, String>> applied = new TreeMap<>();
        Map<String, Object> overrides = new TreeMap<>();
        for (Map.Entry<String, Object> e : ranges.entrySet()) {
            if (playerOverrides.containsKey(e.getKey())) continue;
            if (!excludes(e.getValue(), running, parsePredicate, test)) continue;
            overrides.put(e.getKey(), Map.of("-depends", Map.of("fabricloader", "*")));
            applied.put(e.getKey(), Map.of("fabricloader", describe(e.getValue()), "by", "auto"));
        }
        for (Map.Entry<String, Object> e : playerOverrides.entrySet()) {
            if (e.getValue() instanceof Map<?, ?> m && m.isEmpty()) continue; // "keep as declared"
            overrides.put(e.getKey(), e.getValue());
            applied.put(e.getKey(), Map.of("fabricloader", describe(ranges.get(e.getKey())), "by", "player"));
        }
        if (overrides.isEmpty()) {
            Files.deleteIfExists(file);
            Files.deleteIfExists(stamp);
        } else {
            Map<String, Object> root = new LinkedHashMap<>();
            root.put("version", 1); // Fabric requires it first
            root.put("overrides", overrides);
            String text = Json.write(root);
            Files.writeString(file, text);
            Files.writeString(stamp, Integer.toHexString(text.hashCode()));
        }
        return applied;
    }

    private static String describe(Object range) {
        if (range == null) return "";
        if (range instanceof List<?> l) {
            List<String> parts = new ArrayList<>();
            for (Object o : l) parts.add(String.valueOf(o));
            return String.join(" || ", parts);
        }
        return String.valueOf(range);
    }

    /** Whether a declared range (a string, or a list meaning any of them) leaves the loader out. */
    private static boolean excludes(Object range, Object running, Method parse, Method test) {
        List<Object> any = range instanceof List<?> l ? new ArrayList<>(l) : List.of(range);
        try {
            for (Object predicate : any) {
                if ((Boolean) test.invoke(parse.invoke(null, String.valueOf(predicate)), running)) return false;
            }
            return true;
        } catch (ReflectiveOperationException e) {
            return false; // a range Fabric can't parse: leave it to Fabric's own error
        }
    }

    /** The fabricloader ranges a jar and the jars nested in it declare. */
    private static void readJar(InputStream in, Map<String, Object> ranges) throws IOException {
        ZipInputStream zip = new ZipInputStream(in);
        for (ZipEntry e; (e = zip.getNextEntry()) != null; ) {
            String name = e.getName();
            if (name.equals("fabric.mod.json")) {
                Object meta = MiniJson.parse(new String(zip.readAllBytes(), StandardCharsets.UTF_8));
                if (meta instanceof Map<?, ?> m && m.get("id") instanceof String id
                        && m.get("depends") instanceof Map<?, ?> depends && depends.containsKey("fabricloader")) {
                    ranges.put(id, depends.get("fabricloader"));
                }
            } else if (name.startsWith("META-INF/jars/") && name.endsWith(".jar")) {
                readJar(new ByteArrayInputStream(zip.readAllBytes()), ranges);
            }
        }
    }
}
