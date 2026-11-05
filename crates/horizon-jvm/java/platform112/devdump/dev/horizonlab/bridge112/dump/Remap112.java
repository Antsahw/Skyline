package dev.horizonlab.bridge112.dump;

import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.FieldVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.commons.ClassRemapper;
import org.objectweb.asm.commons.Remapper;

/**
 * Dev tool (tools/java_bridge/build112.sh): lets the 1.12 platform code be written with MCP
 * names, the names 1.12 mods are written in, while the game runs with SRG names.
 *
 * <pre>
 * deobf OUT.jar METHODS.csv FIELDS.csv JAR...     SRG-named classes (the class dump, Forge's
 *                                                 universal jar) renamed to MCP: the compile jar
 * reobf IN_DIR OUT_DIR METHODS.csv FIELDS.csv JAR...  compiled MCP-named classes renamed back to
 *                                                 SRG, each member found through its owner's
 *                                                 supertypes in the SRG jars (what ForgeGradle's
 *                                                 reobf does for mod authors)
 * </pre>
 *
 * Class names are the same in both (FML deobfuscates classes to MCP names at run time); only
 * {@code func_*} methods and {@code field_*} fields differ. The CSVs are MCP's stable_39 export
 * (names only; fetched by build112.sh into the cache, never committed).
 */
public final class Remap112 {
    private Remap112() {}

    public static void main(String[] args) throws IOException {
        int at = args[0].equals("deobf") ? 2 : 3;
        Map<String, String> methods = csv(args[at]);
        Map<String, String> fields = csv(args[at + 1]);
        List<File> jars = new ArrayList<>();
        for (int i = at + 2; i < args.length; i++) jars.add(new File(args[i]));
        if (args[0].equals("deobf")) deobf(new File(args[1]), methods, fields, jars);
        else if (args[0].equals("reobf")) reobf(new File(args[1]), new File(args[2]), methods, fields, jars);
        else throw new IllegalArgumentException(args[0]);
    }

    static Map<String, String> csv(String path) throws IOException {
        Map<String, String> out = new HashMap<>();
        try (BufferedReader r = Files.newBufferedReader(new File(path).toPath(), StandardCharsets.UTF_8)) {
            r.readLine();
            for (String line; (line = r.readLine()) != null; ) {
                String[] p = line.split(",", 3);
                if (p.length >= 2) out.put(p[0], p[1]);
            }
        }
        return out;
    }

    // ---- deobf ----

    static void deobf(File out, Map<String, String> methods, Map<String, String> fields, List<File> jars) throws IOException {
        Remapper toMcp = new Remapper() {
            @Override
            public String mapMethodName(String owner, String name, String desc) {
                return methods.getOrDefault(name, name);
            }

            @Override
            public String mapFieldName(String owner, String name, String desc) {
                return fields.getOrDefault(name, name);
            }
        };
        int n = 0;
        try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(out))) {
            java.util.Set<String> seen = new java.util.HashSet<>();
            for (File jar : jars) {
                try (ZipFile z = new ZipFile(jar)) {
                    for (ZipEntry e : java.util.Collections.list(z.entries())) {
                        if (!e.getName().endsWith(".class") || !seen.add(e.getName())) continue;
                        ClassReader reader = new ClassReader(read(z.getInputStream(e)));
                        ClassWriter writer = new ClassWriter(0);
                        reader.accept(new ClassRemapper(writer, toMcp), 0);
                        zip.putNextEntry(new ZipEntry(e.getName()));
                        zip.write(writer.toByteArray());
                        zip.closeEntry();
                        n++;
                    }
                }
            }
        }
        System.out.println("deobf: " + n + " classes -> " + out);
    }

    // ---- reobf ----

    /** A class's supertypes and its members' SRG names keyed by MCP name (+ descriptor). */
    static final class Info {
        String superName;
        String[] interfaces = new String[0];
        final Map<String, String> methods = new HashMap<>(); // mcpName + desc -> srg
        final Map<String, String> fields = new HashMap<>(); // mcpName -> srg
    }

    static void reobf(File in, File out, Map<String, String> methods, Map<String, String> fields, List<File> jars) throws IOException {
        Map<String, Info> infos = new HashMap<>();
        for (File jar : jars) {
            try (ZipFile z = new ZipFile(jar)) {
                for (ZipEntry e : java.util.Collections.list(z.entries())) {
                    if (e.getName().endsWith(".class")) index(read(z.getInputStream(e)), infos, methods, fields);
                }
            }
        }
        List<File> own = new ArrayList<>();
        collect(in, own);
        for (File f : own) index(Files.readAllBytes(f.toPath()), infos, methods, fields);
        Remapper toSrg = new Remapper() {
            @Override
            public String mapMethodName(String owner, String name, String desc) {
                String srg = findMethod(infos, owner, name + desc, new java.util.HashSet<>());
                return srg != null ? srg : name;
            }

            @Override
            public String mapFieldName(String owner, String name, String desc) {
                String srg = findField(infos, owner, name, new java.util.HashSet<>());
                return srg != null ? srg : name;
            }
        };
        for (File f : own) {
            ClassReader reader = new ClassReader(Files.readAllBytes(f.toPath()));
            ClassWriter writer = new ClassWriter(0);
            reader.accept(new ClassRemapper(writer, toSrg), 0);
            File target = new File(out, in.toPath().relativize(f.toPath()).toString());
            target.getParentFile().mkdirs();
            Files.write(target.toPath(), writer.toByteArray());
        }
        System.out.println("reobf: " + own.size() + " classes -> " + out);
    }

    static String findMethod(Map<String, Info> infos, String owner, String key, java.util.Set<String> seen) {
        Info info = infos.get(owner);
        if (info == null || !seen.add(owner)) return null;
        String srg = info.methods.get(key);
        if (srg != null) return srg;
        if (info.superName != null) {
            srg = findMethod(infos, info.superName, key, seen);
            if (srg != null) return srg;
        }
        for (String i : info.interfaces) {
            srg = findMethod(infos, i, key, seen);
            if (srg != null) return srg;
        }
        return null;
    }

    static String findField(Map<String, Info> infos, String owner, String name, java.util.Set<String> seen) {
        Info info = infos.get(owner);
        if (info == null || !seen.add(owner)) return null;
        String srg = info.fields.get(name);
        if (srg != null) return srg;
        if (info.superName != null) {
            srg = findField(infos, info.superName, name, seen);
            if (srg != null) return srg;
        }
        for (String i : info.interfaces) {
            srg = findField(infos, i, name, seen);
            if (srg != null) return srg;
        }
        return null;
    }

    static void index(byte[] bytes, Map<String, Info> infos, Map<String, String> methods, Map<String, String> fields) {
        ClassReader reader = new ClassReader(bytes);
        Info info = new Info();
        reader.accept(new ClassVisitor(Opcodes.ASM9) {
            @Override
            public void visit(int version, int access, String name, String signature, String superName, String[] interfaces) {
                info.superName = superName;
                if (interfaces != null) info.interfaces = interfaces;
                infos.putIfAbsent(name, info);
            }

            @Override
            public MethodVisitor visitMethod(int access, String name, String desc, String signature, String[] exceptions) {
                String mcp = methods.get(name);
                if (mcp != null) info.methods.put(mcp + desc, name);
                return null;
            }

            @Override
            public FieldVisitor visitField(int access, String name, String desc, String signature, Object value) {
                String mcp = fields.get(name);
                if (mcp != null) info.fields.put(mcp, name);
                return null;
            }
        }, ClassReader.SKIP_CODE);
    }

    static void collect(File dir, List<File> out) {
        File[] files = dir.listFiles();
        if (files == null) return;
        for (File f : files) {
            if (f.isDirectory()) collect(f, out);
            else if (f.getName().endsWith(".class")) out.add(f);
        }
    }

    static byte[] read(InputStream in) throws IOException {
        try (InputStream s = in) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[1 << 16];
            for (int n; (n = s.read(buf)) > 0; ) out.write(buf, 0, n);
            return out.toByteArray();
        }
    }
}
