package dev.horizonlab.build120;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

import net.fabricmc.mappingio.MappingReader;
import net.fabricmc.mappingio.MappingWriter;
import net.fabricmc.mappingio.adapter.MappingNsRenamer;
import net.fabricmc.mappingio.adapter.MappingSourceNsSwitch;
import net.fabricmc.mappingio.format.MappingFormat;
import net.fabricmc.mappingio.tree.MemoryMappingTree;
import net.fabricmc.tinyremapper.NonClassCopyMode;
import net.fabricmc.tinyremapper.OutputConsumerPath;
import net.fabricmc.tinyremapper.TinyRemapper;
import net.fabricmc.tinyremapper.TinyUtils;

/**
 * The bridge's build tool for its 1.20.1 code (tools/java_bridge/build120.sh): what Fabric's
 * Loom does for a mod, without Gradle. Runs on the build machine with Fabric's tiny-remapper
 * (its fat jar holds ASM and mapping-io), never in a game.
 *
 * <pre>
 * Remap120 mappings INTERMEDIARY_JAR MOJANG_TXT OUT.tiny
 *     official, intermediary and named (Mojang's names) in one Tiny v2 file
 * Remap120 remap MAPPINGS.tiny FROM TO IN.jar OUT.jar [CLASSPATH_JAR...]
 *     a jar from one namespace to another (classes, and resources as they are)
 * </pre>
 *
 * Mojang's 1.20.1 jar remapped to {@code named} is what platform120 compiles against; the
 * compiled classes are remapped {@code named} to {@code intermediary}, the names 1.20.1 runs
 * under in Fabric. Both outputs hold or derive from Mojang's code: they stay in the cache.
 */
public final class Remap120 {
    private Remap120() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 4 && args[0].equals("mappings")) {
            mappings(Path.of(args[1]), Path.of(args[2]), Path.of(args[3]));
        } else if (args.length >= 6 && args[0].equals("remap")) {
            List<Path> classPath = new ArrayList<>();
            for (int i = 6; i < args.length; i++) classPath.add(Path.of(args[i]));
            remap(Path.of(args[1]), args[2], args[3], Path.of(args[4]), Path.of(args[5]), classPath);
        } else {
            System.err.println("usage: Remap120 mappings INTERMEDIARY_JAR MOJANG_TXT OUT.tiny");
            System.err.println("       Remap120 remap MAPPINGS.tiny FROM TO IN.jar OUT.jar [CLASSPATH_JAR...]");
            System.exit(2);
        }
    }

    static void mappings(Path intermediaryJar, Path mojang, Path out) throws IOException {
        Path tiny = Files.createTempFile("intermediary", ".tiny");
        try {
            extract(intermediaryJar, "mappings/mappings.tiny", tiny);
            MemoryMappingTree tree = new MemoryMappingTree();
            // official -> intermediary
            MappingReader.read(tiny, MappingFormat.TINY_2_FILE, tree);
            // Mojang's file maps their names (its "source") to official (its "target"): renamed,
            // then switched so official is the source, it merges in as a third namespace.
            MemoryMappingTree proguard = new MemoryMappingTree();
            MappingReader.read(mojang, MappingFormat.PROGUARD_FILE, proguard);
            proguard.accept(new MappingNsRenamer(new MappingSourceNsSwitch(tree, "official"),
                    Map.of("source", "named", "target", "official")));
            Files.createDirectories(out.toAbsolutePath().getParent());
            try (var writer = MappingWriter.create(out, MappingFormat.TINY_2_FILE)) {
                tree.accept(writer);
            }
            System.out.println("mappings: " + tree.getClasses().size() + " classes, namespaces " + tree.getSrcNamespace() + " " + tree.getDstNamespaces());
        } finally {
            Files.deleteIfExists(tiny);
        }
    }

    private static void extract(Path jar, String entry, Path to) throws IOException {
        try (InputStream in = Files.newInputStream(jar); ZipInputStream zip = new ZipInputStream(in)) {
            for (ZipEntry e; (e = zip.getNextEntry()) != null; ) {
                if (e.getName().equals(entry)) {
                    Files.write(to, zip.readAllBytes());
                    return;
                }
            }
        }
        throw new IOException(entry + " is not in " + jar);
    }

    static void remap(Path mappings, String from, String to, Path in, Path out, List<Path> classPath) throws IOException {
        TinyRemapper remapper = TinyRemapper.newRemapper()
                .withMappings(TinyUtils.createTinyMappingProvider(mappings, from, to))
                .renameInvalidLocals(true)
                .rebuildSourceFilenames(true)
                .build();
        Files.deleteIfExists(out);
        Files.createDirectories(out.toAbsolutePath().getParent());
        try (OutputConsumerPath consumer = new OutputConsumerPath.Builder(out).build()) {
            consumer.addNonClassFiles(in, NonClassCopyMode.FIX_META_INF, remapper);
            remapper.readInputs(in);
            remapper.readClassPath(classPath.toArray(new Path[0]));
            remapper.apply(consumer);
        } finally {
            remapper.finish();
        }
        System.out.println("remapped " + in.getFileName() + " " + from + " -> " + to + ": " + out);
    }
}
