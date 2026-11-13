package dev.horizonlab.platform120.mixin;

import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

/**
 * The headless client's in-game GUI, made without its constructor (Hud120): what its parts read,
 * and the parts mods draw into.
 */
@Mixin(Gui.class)
public interface GuiAccess {
    @Mutable
    @Accessor("minecraft")
    void horizon$setMinecraft(net.minecraft.client.Minecraft minecraft);

    @Mutable
    @Accessor("random")
    void horizon$setRandom(net.minecraft.util.RandomSource random);

    @Mutable
    @Accessor("itemRenderer")
    void horizon$setItemRenderer(net.minecraft.client.renderer.entity.ItemRenderer items);

    @Accessor("screenWidth")
    void horizon$setScreenWidth(int width);

    @Accessor("screenHeight")
    void horizon$setScreenHeight(int height);

    @Accessor("tickCount")
    int horizon$tickCount();

    @Accessor("tickCount")
    void horizon$setTickCount(int ticks);

    @Invoker("renderPlayerHealth")
    void horizon$renderPlayerHealth(GuiGraphics graphics);

    @Invoker("renderHotbar")
    void horizon$renderHotbar(float partialTick, GuiGraphics graphics);

    @Invoker("renderExperienceBar")
    void horizon$renderExperienceBar(GuiGraphics graphics, int x);

    @Invoker("renderEffects")
    void horizon$renderEffects(GuiGraphics graphics);
}
