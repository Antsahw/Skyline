package dev.horizonlab.platform120.mixin;

import net.minecraft.client.StringSplitter;
import net.minecraft.client.gui.Font;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A font made without its constructor: what measuring text needs, and nothing that draws. */
@Mixin(Font.class)
public interface FontAccess {
    @Mutable
    @Accessor("lineHeight")
    void horizon$setLineHeight(int height);

    @Mutable
    @Accessor("splitter")
    void horizon$setSplitter(StringSplitter splitter);
}
