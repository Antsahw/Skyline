package dev.horizonlab.bridge112.dump;

import java.io.FileOutputStream;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Dev tool (tools/java_bridge/build112.sh), never shipped: a Java agent that keeps the final
 * bytes of every {@code net/minecraft/} and {@code net/minecraftforge/} class the game's class
 * loader defines (Mojang's classes after FML's deobfuscation to SRG names and Cleanroom's patches;
 * Forge's, whose references to Mojang's classes are deobfuscated the same way) and writes them to a jar when
 * the VM exits. The bridge's 1.12 platform code is compiled against that jar, the way
 * ForgeGradle's dev jar serves mod authors. The jar holds Mojang's code: it stays in the user's
 * cache, never in the repo.
 *
 * {@code -javaagent:devdump.jar=OUT.jar}; {@link DumpMod} loads every game class, then exits.
 */
public final class ClassDumpAgent {
    private ClassDumpAgent() {}

    static final Map<String, byte[]> CLASSES = new ConcurrentHashMap<>();

    public static void premain(String out, Instrumentation inst) {
        inst.addTransformer(new ClassFileTransformer() {
            @Override
            public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain pd, byte[] bytes) {
                if (name != null && loader != null && (name.startsWith("net/minecraft/") || name.startsWith("net/minecraftforge/")) && !name.startsWith("net/minecraft/launchwrapper/")) {
                    CLASSES.put(name, bytes.clone());
                }
                return null;
            }
        });
        Runtime.getRuntime().addShutdownHook(new Thread(() -> write(out), "horizon112 class dump"));
    }

    private static void write(String out) {
        try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(out))) {
            for (Map.Entry<String, byte[]> e : new java.util.TreeMap<>(CLASSES).entrySet()) {
                zip.putNextEntry(new ZipEntry(e.getKey() + ".class"));
                zip.write(e.getValue());
                zip.closeEntry();
            }
            System.out.println("[horizon112] dumped " + CLASSES.size() + " classes to " + out);
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
