package dev.horizonlab.platform.mixin;

import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.FieldInsnNode;
import org.objectweb.asm.tree.InsnList;
import org.objectweb.asm.tree.InsnNode;
import org.objectweb.asm.tree.JumpInsnNode;
import org.objectweb.asm.tree.LabelNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.MethodNode;
import org.objectweb.asm.tree.VarInsnNode;
import org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;

/**
 * Horizon's platform mixins, plus one change Mixin can't write: every GuiGraphicsExtractor
 * method HorizonGraphics records (and {@code pose()}) starts by handing the call to the
 * HorizonGraphics whose render state the extractor draws on, when it isn't that HorizonGraphics
 * itself. A mod that makes its own extractor on the one it was given (a subclass built from its
 * fields: REI's compat GuiGraphics, any mod's) then records into Horizon's frame as the screen
 * does, whatever the mod.
 */
public final class PlatformPlugin implements IMixinConfigPlugin {
    private static final String EXTRACTOR = "net/minecraft/client/gui/GuiGraphicsExtractor";
    private static final String GRAPHICS = "dev/horizonlab/platform/HorizonGraphics";
    private static final String WRAPPERS = "dev/horizonlab/platform/HorizonGraphicsWrappers";

    @Override
    public void onLoad(String mixinPackage) {}

    @Override
    public String getRefMapperConfig() {
        return null;
    }

    @Override
    public boolean shouldApplyMixin(String targetClassName, String mixinClassName) {
        return true;
    }

    @Override
    public void acceptTargets(Set<String> myTargets, Set<String> otherTargets) {}

    @Override
    public List<String> getMixins() {
        return null;
    }

    @Override
    public void preApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {}

    @Override
    public void postApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {
        if (!targetClass.name.equals(EXTRACTOR) || !mixinClassName.endsWith(".GuiGraphicsMixin")) return;
        Set<String> recorded = recorded();
        String stateDesc = null;
        for (var f : targetClass.fields) if (f.name.equals("guiRenderState")) stateDesc = f.desc;
        if (stateDesc == null) return;
        for (MethodNode m : targetClass.methods) {
            if ((m.access & (Opcodes.ACC_STATIC | Opcodes.ACC_FINAL | Opcodes.ACC_PRIVATE | Opcodes.ACC_ABSTRACT)) != 0) continue;
            if (m.name.startsWith("<") || m.name.contains("$")) continue;
            if (!recorded.contains(m.name + m.desc) && !(m.name.equals("pose") && m.desc.startsWith("()"))) continue;
            m.instructions.insert(forward(m, stateDesc));
        }
    }

    /** if ((h = HorizonGraphicsWrappers.forward(this, this.guiRenderState)) != null) return h.m(args); */
    private static InsnList forward(MethodNode m, String stateDesc) {
        InsnList in = new InsnList();
        LabelNode own = new LabelNode();
        in.add(new VarInsnNode(Opcodes.ALOAD, 0));
        in.add(new VarInsnNode(Opcodes.ALOAD, 0));
        in.add(new FieldInsnNode(Opcodes.GETFIELD, EXTRACTOR, "guiRenderState", stateDesc));
        in.add(new MethodInsnNode(Opcodes.INVOKESTATIC, WRAPPERS, "forward", "(Ljava/lang/Object;Ljava/lang/Object;)L" + GRAPHICS + ";", false));
        in.add(new InsnNode(Opcodes.DUP));
        in.add(new JumpInsnNode(Opcodes.IFNULL, own));
        int slot = 1;
        for (Type arg : Type.getArgumentTypes(m.desc)) {
            in.add(new VarInsnNode(arg.getOpcode(Opcodes.ILOAD), slot));
            slot += arg.getSize();
        }
        in.add(new MethodInsnNode(Opcodes.INVOKEVIRTUAL, GRAPHICS, m.name, m.desc, false));
        in.add(new InsnNode(Type.getReturnType(m.desc).getOpcode(Opcodes.IRETURN)));
        in.add(own);
        in.add(new InsnNode(Opcodes.POP));
        return in;
    }

    /** HorizonGraphics' own public instance methods (read, not loaded: it extends the class being made). */
    private static Set<String> recorded() {
        Set<String> out = new HashSet<>();
        try (var in = PlatformPlugin.class.getClassLoader().getResourceAsStream(GRAPHICS + ".class")) {
            if (in == null) return out;
            ClassNode node = new ClassNode();
            new ClassReader(in.readAllBytes()).accept(node, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
            for (MethodNode m : node.methods) {
                if ((m.access & Opcodes.ACC_PUBLIC) != 0 && (m.access & (Opcodes.ACC_STATIC | Opcodes.ACC_SYNTHETIC | Opcodes.ACC_BRIDGE)) == 0) {
                    out.add(m.name + m.desc);
                }
            }
        } catch (Exception e) {
            // Nothing forwarded: mods' own extractors draw as before.
        }
        return out;
    }
}
