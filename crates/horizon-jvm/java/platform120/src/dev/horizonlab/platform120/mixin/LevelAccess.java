package dev.horizonlab.platform120.mixin;

import net.minecraft.core.Holder;
import net.minecraft.resources.ResourceKey;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.dimension.DimensionType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** What Level's constructor sets that a mirror level (Mirror120.level) sets itself. */
@Mixin(Level.class)
public interface LevelAccess {
    @Mutable
    @Accessor("dimension")
    void horizon$setDimension(ResourceKey<Level> dimension);

    @Mutable
    @Accessor("random")
    void horizon$setRandom(RandomSource random);

    @Mutable
    @Accessor("dimensionTypeRegistration")
    void horizon$setDimensionType(Holder<DimensionType> type);

    @Mutable
    @Accessor("threadSafeRandom")
    void horizon$setThreadSafeRandom(RandomSource random);

    @Mutable
    @Accessor("worldBorder")
    void horizon$setWorldBorder(net.minecraft.world.level.border.WorldBorder border);

    @Mutable
    @Accessor("levelData")
    void horizon$setLevelData(net.minecraft.world.level.storage.WritableLevelData data);
}
