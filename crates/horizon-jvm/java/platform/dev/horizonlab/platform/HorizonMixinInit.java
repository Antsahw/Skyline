package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Handle;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.AbstractInsnNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.FieldInsnNode;
import org.objectweb.asm.tree.FrameNode;
import org.objectweb.asm.tree.InsnList;
import org.objectweb.asm.tree.InvokeDynamicInsnNode;
import org.objectweb.asm.tree.LabelNode;
import org.objectweb.asm.tree.LdcInsnNode;
import org.objectweb.asm.tree.LineNumberNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.MethodNode;
import org.objectweb.asm.tree.TryCatchBlockNode;
import org.objectweb.asm.tree.TypeInsnNode;

/**
 * The mixins' field initializers, run on objects the mirror makes without their constructor.
 * Mixin merges a mixin's initializers (the code of its constructor after its super call:
 * {@code private final LocalPlayerAim aim = new LocalPlayerAim(this.data, this.player);}) into
 * every constructor of the target, which those objects skip. Here that code is copied as it
 * is, the mixin's own names made the target's, into a static method of a hidden class nested
 * with the target (so it reaches the target's private fields as the merged code does), and run
 * on the object. HorizonHooks.initFields' patterns (a no-argument {@code new}, a static getter)
 * still cover what can't be copied (a lambda of the mixin's, which Mixin renames when it merges).
 */
public final class HorizonMixinInit {
    private HorizonMixinInit() {}

    /** A mixin's initializer: the mixin, its code after the super call, the fields it writes. */
    private record Init(String mixin, MethodNode ctor, int from, List<String> writes) {}

    /** Initializers by target class (internal name). */
    private static final Map<String, List<Init>> BY_TARGET = new ConcurrentHashMap<>();
    /** The copied initializer of (target, mixin), or NONE when it can't be. */
    private static final Map<String, MethodHandle> MADE = new ConcurrentHashMap<>();
    private static final java.util.Set<String> RAN = ConcurrentHashMap.newKeySet();
    private static final MethodHandle NONE = MethodHandles.constant(Object.class, null);

    /** Reads a mixin class's initializer (HorizonHooks' scan of the mods' mixin classes). */
    static void read(ClassNode node, List<String> targets) {
        for (MethodNode m : node.methods) {
            if (!m.name.equals("<init>") || !m.desc.equals("()V")) continue;
            int from = -1;
            for (int i = 0; i < m.instructions.size(); i++) {
                if (m.instructions.get(i) instanceof MethodInsnNode c && c.getOpcode() == Opcodes.INVOKESPECIAL
                        && c.name.equals("<init>") && c.owner.equals(node.superName)) {
                    from = i + 1;
                    break;
                }
            }
            if (from < 0) continue;
            List<String> writes = new ArrayList<>();
            for (int i = from; i < m.instructions.size(); i++) {
                if (m.instructions.get(i) instanceof FieldInsnNode f && f.getOpcode() == Opcodes.PUTFIELD
                        && f.owner.equals(node.name) && !writes.contains(f.name)) {
                    writes.add(f.name);
                }
            }
            if (writes.isEmpty()) continue;
            for (String t : targets) BY_TARGET.computeIfAbsent(t, k -> new ArrayList<>()).add(new Init(node.name, m, from, writes));
        }
    }

    /**
     * Runs the initializers of the mixins on {@code owner} (one of {@code self}'s classes) whose
     * fields are all still unset (a reference null): an object made without its constructor.
     */
    static void run(Object self, Class<?> owner) {
        HorizonHooks.scanNow();
        List<Init> inits = BY_TARGET.get(owner.getName().replace('.', '/'));
        if (inits == null) return;
        for (Init init : inits) {
            if (!unset(self, owner, init.writes())) continue;
            MethodHandle handle = MADE.computeIfAbsent(owner.getName() + "/" + init.mixin(), k -> make(owner, init));
            if (handle == NONE) continue;
            try {
                handle.invoke(self);
                if (RAN.add(owner.getName() + "/" + init.mixin())) {
                    Natives.log("Mixin fields: " + init.mixin() + "'s initializer ran on a " + owner.getSimpleName() + " (" + init.writes().size() + " fields)");
                }
            } catch (Throwable t) {
                // Its code may need what the object lacks (a constructor argument): the fields
                // left unset go on to HorizonHooks' patterns.
                if (HorizonHooks.DEBUG) Natives.log("Mixin fields: " + init.mixin() + "'s initializer on " + owner.getSimpleName() + " failed: " + t);
            }
        }
    }

    private static boolean isFinal(Class<?> owner, String name) {
        try {
            return java.lang.reflect.Modifier.isFinal(owner.getDeclaredField(name).getModifiers());
        } catch (NoSuchFieldException e) {
            return false;
        }
    }

    private static String box(Type t) {
        return switch (t.getSort()) {
            case Type.BOOLEAN -> "Ljava/lang/Boolean;";
            case Type.CHAR -> "Ljava/lang/Character;";
            case Type.BYTE -> "Ljava/lang/Byte;";
            case Type.SHORT -> "Ljava/lang/Short;";
            case Type.INT -> "Ljava/lang/Integer;";
            case Type.FLOAT -> "Ljava/lang/Float;";
            case Type.LONG -> "Ljava/lang/Long;";
            default -> "Ljava/lang/Double;";
        };
    }

    private static final Map<String, java.lang.reflect.Field> FIELDS = new ConcurrentHashMap<>();

    /** A copied initializer's write of a final field (the object's class or a superclass declares it). */
    public static void put(Object self, Object value, String name) throws ReflectiveOperationException {
        var field = FIELDS.get(self.getClass().getName() + "." + name);
        if (field == null) {
            for (Class<?> c = self.getClass(); c != null && field == null; c = c.getSuperclass()) {
                try {
                    field = c.getDeclaredField(name);
                } catch (NoSuchFieldException e) {
                    // Further up.
                }
            }
            if (field == null) throw new NoSuchFieldException(name);
            field.setAccessible(true);
            FIELDS.put(self.getClass().getName() + "." + name, field);
        }
        field.set(self, value);
    }

    private static boolean unset(Object self, Class<?> owner, List<String> fields) {
        boolean any = false;
        for (String name : fields) {
            try {
                var field = owner.getDeclaredField(name);
                if (java.lang.reflect.Modifier.isStatic(field.getModifiers())) continue;
                if (field.getType().isPrimitive()) continue;
                field.setAccessible(true);
                if (field.get(self) != null) return false;
                any = true;
            } catch (ReflectiveOperationException | RuntimeException e) {
                // Mixin renamed it (a conflict): the copy wouldn't link.
                return false;
            }
        }
        return any;
    }

    private static MethodHandle make(Class<?> owner, Init init) {
        String target = owner.getName().replace('.', '/');
        try {
            byte[] bytes = copy(target, init, owner);
            if (bytes == null) return NONE;
            var lookup = MethodHandles.privateLookupIn(owner, MethodHandles.lookup())
                    .defineHiddenClass(bytes, true, MethodHandles.Lookup.ClassOption.NESTMATE);
            return lookup.findStatic(lookup.lookupClass(), "init", MethodType.methodType(void.class, owner));
        } catch (Throwable t) {
            Natives.log("Mixin fields: " + init.mixin() + "'s initializer can't run on " + owner.getSimpleName() + ": " + t);
            return NONE;
        }
    }

    /** The class holding the copied initializer, or null when its code can't be copied. */
    private static byte[] copy(String target, Init init, Class<?> owner) {
        ClassLoader loader = owner.getClassLoader();
        String mixin = init.mixin();
        String pkg = target.contains("/") ? target.substring(0, target.lastIndexOf('/') + 1) : "";
        ClassNode out = new ClassNode();
        out.version = Opcodes.V21;
        out.access = Opcodes.ACC_FINAL | Opcodes.ACC_SUPER | Opcodes.ACC_SYNTHETIC;
        out.name = pkg + "HorizonMixinInit";
        out.superName = "java/lang/Object";
        MethodNode m = new MethodNode(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "init", "(L" + target + ";)V", null, null);
        Map<LabelNode, LabelNode> labels = new HashMap<>();
        for (var insn : init.ctor().instructions) if (insn instanceof LabelNode l) labels.put(l, new LabelNode());
        InsnList code = new InsnList();
        for (int i = init.from(); i < init.ctor().instructions.size(); i++) {
            AbstractInsnNode insn = init.ctor().instructions.get(i);
            if (insn instanceof LineNumberNode || insn instanceof FrameNode) continue;
            AbstractInsnNode c = insn.clone(labels);
            if (c instanceof FieldInsnNode f) {
                if (f.owner.equals(mixin)) f.owner = target;
                f.desc = f.desc.replace("L" + mixin + ";", "L" + target + ";");
                // A final field is written only by its class's constructor: through reflection
                // (put), its value boxed.
                if (f.getOpcode() == Opcodes.PUTFIELD && f.owner.equals(target) && isFinal(owner, f.name)) {
                    Type type = Type.getType(f.desc);
                    if (type.getSort() < Type.ARRAY) {
                        Type boxed = Type.getType(box(type));
                        code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, boxed.getInternalName(), "valueOf",
                                "(" + type.getDescriptor() + ")" + boxed.getDescriptor(), false));
                    }
                    code.add(new LdcInsnNode(f.name));
                    code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, "dev/horizonlab/platform/HorizonMixinInit", "put",
                            "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/String;)V", false));
                    continue;
                }
            } else if (c instanceof MethodInsnNode mi) {
                if (mi.owner.equals(mixin)) {
                    mi.owner = target;
                    // A merged private method, called from a nestmate.
                    if (mi.getOpcode() == Opcodes.INVOKESPECIAL && !mi.name.equals("<init>")) mi.setOpcode(Opcodes.INVOKEVIRTUAL);
                }
                mi.desc = mi.desc.replace("L" + mixin + ";", "L" + target + ";");
            } else if (c instanceof TypeInsnNode t) {
                if (t.desc.equals(mixin)) t.desc = target;
            } else if (c instanceof LdcInsnNode l && l.cst instanceof Type t && t.getInternalName().equals(mixin)) {
                l.cst = Type.getObjectType(target);
            } else if (c instanceof InvokeDynamicInsnNode d) {
                // The mixin's own lambdas are merged under other names.
                for (Object a : d.bsmArgs) if (a instanceof Handle h && h.getOwner().equals(mixin)) return null;
            }
            code.add(c);
        }
        m.instructions = code;
        for (TryCatchBlockNode b : init.ctor().tryCatchBlocks) {
            int start = init.ctor().instructions.indexOf(b.start);
            if (start < init.from()) continue;
            m.tryCatchBlocks.add(new TryCatchBlockNode(labels.get(b.start), labels.get(b.end), labels.get(b.handler), b.type));
        }
        out.methods.add(m);
        ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_FRAMES | ClassWriter.COMPUTE_MAXS) {
            @Override
            protected String getCommonSuperClass(String a, String b) {
                try {
                    Class<?> x = Class.forName(a.replace('/', '.'), false, loader);
                    Class<?> y = Class.forName(b.replace('/', '.'), false, loader);
                    if (x.isAssignableFrom(y)) return a;
                    if (y.isAssignableFrom(x)) return b;
                    if (x.isInterface() || y.isInterface()) return "java/lang/Object";
                    do {
                        x = x.getSuperclass();
                    } while (!x.isAssignableFrom(y));
                    return x.getName().replace('.', '/');
                } catch (Throwable t) {
                    return "java/lang/Object";
                }
            }
        };
        out.accept(writer);
        return writer.toByteArray();
    }
}
