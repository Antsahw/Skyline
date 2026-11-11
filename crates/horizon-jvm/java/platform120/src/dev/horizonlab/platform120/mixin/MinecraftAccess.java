package dev.horizonlab.platform120.mixin;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The headless client's font and options, which a screen reads as it is laid out and used. */
@Mixin(Minecraft.class)
public interface MinecraftAccess {
    @Mutable
    @Accessor("font")
    void horizon$setFont(Font font);

    @Mutable
    @Accessor("options")
    void horizon$setOptions(net.minecraft.client.Options options);
}
