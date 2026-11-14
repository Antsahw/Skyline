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
    @Accessor("blockColors")
    void horizon$setBlockColors(net.minecraft.client.color.block.BlockColors colors);

    @Mutable
    @Accessor("options")
    void horizon$setOptions(net.minecraft.client.Options options);

    @Mutable
    @Accessor("blockRenderer")
    void horizon$setBlockRenderer(net.minecraft.client.renderer.block.BlockRenderDispatcher blocks);

    @Mutable
    @Accessor("mouseHandler")
    void horizon$setMouseHandler(net.minecraft.client.MouseHandler mouse);

    @Mutable
    @Accessor("itemRenderer")
    void horizon$setItemRenderer(net.minecraft.client.renderer.entity.ItemRenderer items);

    @Mutable
    @Accessor("soundManager")
    void horizon$setSoundManager(net.minecraft.client.sounds.SoundManager sounds);

    @Accessor("lastInputType")
    void horizon$setLastInputType(net.minecraft.client.InputType type);

    @Mutable
    @Accessor("gui")
    void horizon$setGui(net.minecraft.client.gui.Gui gui);

    @Accessor("profiler")
    void horizon$setProfiler(net.minecraft.util.profiling.ProfilerFiller profiler);

    /** Its frame timer (Jade's HUD asks the frame's delta). */
    @Mutable
    @Accessor("timer")
    void horizon$setTimer(net.minecraft.client.Timer timer);

    /** The client's baked models (Models120): mods' models fetch extra parts from it. */
    @Mutable
    @Accessor("modelManager")
    void horizon$setModelManager(net.minecraft.client.resources.model.ModelManager manager);

    /** The client's entity model layers (Render120): renderers ask the client for them. */
    @Mutable
    @Accessor("entityModels")
    void horizon$setEntityModels(net.minecraft.client.model.geom.EntityModelSet models);
}
