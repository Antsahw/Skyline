package dev.horizonlab.platform120.mixin;

import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.NaturalSpawner;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** Where a chunk-generation spawn of a mod's type stands: vanilla's own surface search. */
@Mixin(NaturalSpawner.class)
public interface NaturalSpawnerAccess {
    @Invoker("getTopNonCollidingPos")
    static BlockPos horizon$topNonColliding(LevelReader level, EntityType<?> type, int x, int z) {
        throw new AssertionError();
    }
}
