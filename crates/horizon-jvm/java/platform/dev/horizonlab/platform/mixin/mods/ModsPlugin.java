package dev.horizonlab.platform.mixin.mods;

import java.util.List;
import java.util.Map;
import java.util.Set;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.FieldNode;
import org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;

/**
 * Mods' static final fields that hold one client's state, made non-final as their classes
 * load, so each player's client gets its own (HorizonClientState swaps them). A final static field may be folded into
 * compiled code, so it can't be swapped while final. The mixins here are empty, one per class.
 */
public final class ModsPlugin implements IMixinConfigPlugin {
    /** Class (internal name) to its fields made non-final. */
    static final Map<String, Set<String>> FIELDS = Map.of(
            // REI's search executor (HorizonRecipeViewer.searchPerPlayer).
            "me/shedaniel/rei/impl/client/search/AsyncSearchManager", Set.of("EXECUTOR_SERVICE"));

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
        Set<String> names = FIELDS.get(targetClass.name);
        if (names == null) return;
        for (FieldNode field : targetClass.fields) {
            if (names.contains(field.name)) field.access &= ~Opcodes.ACC_FINAL;
        }
    }
}
