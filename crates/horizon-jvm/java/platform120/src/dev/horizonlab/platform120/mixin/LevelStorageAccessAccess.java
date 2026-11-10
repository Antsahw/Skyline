package dev.horizonlab.platform120.mixin;

import net.minecraft.world.level.storage.LevelStorageSource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * Where a mirror level's files are. A mod given the server's save access at the end of a
 * constructor asks it for the dimension's folder and keeps its own data there (GraphLib does,
 * so Wired Redstone does): the folder is the only part of a save access the mirror needs, so
 * the rest of the object stays as allocated.
 */
@Mixin(LevelStorageSource.LevelStorageAccess.class)
public interface LevelStorageAccessAccess {
    @Mutable
    @Accessor("levelDirectory")
    void horizon$setLevelDirectory(LevelStorageSource.LevelDirectory directory);
}
