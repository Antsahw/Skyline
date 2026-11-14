package dev.horizonlab.platform120.mixin;

import net.minecraft.tags.TagKey;
import net.minecraft.world.item.DiggerItem;
import net.minecraft.world.level.block.Block;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The blocks a mod's tool mines fast (Content120: its 26.3 tool rules). */
@Mixin(DiggerItem.class)
public interface DiggerItemAccess {
    @Accessor("blocks")
    TagKey<Block> horizon$blocks();
}
