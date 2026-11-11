package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * A recording GuiGraphics' pose: screens draw their slots and labels relative to an origin
 * they translate to, so the recorder needs a real pose stack to know where things went.
 */
@Mixin(GuiGraphics.class)
public interface GuiGraphicsAccess {
    @Mutable
    @Accessor("pose")
    void horizon$setPose(PoseStack pose);

    @Mutable
    @Accessor("minecraft")
    void horizon$setMinecraft(Minecraft minecraft);
}
