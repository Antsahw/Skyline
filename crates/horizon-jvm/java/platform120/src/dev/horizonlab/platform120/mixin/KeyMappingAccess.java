package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyMapping;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The key a mapping is bound to now (Keys120 tells Horizon's players which keys mods bind). */
@Mixin(KeyMapping.class)
public interface KeyMappingAccess {
    @Accessor("key")
    InputConstants.Key horizon$key();
}
