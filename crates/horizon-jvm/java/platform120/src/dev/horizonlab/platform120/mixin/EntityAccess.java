package dev.horizonlab.platform120.mixin;

import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The HUD's client player is the server player's state under the client's class (Hud120): its
 * synced data (health, air), its level, what its eyes are in.
 */
@Mixin(Entity.class)
public interface EntityAccess {
    @Mutable
    @Accessor("entityData")
    void horizon$setEntityData(net.minecraft.network.syncher.SynchedEntityData data);

    @Accessor("position")
    void horizon$setPosition(net.minecraft.world.phys.Vec3 position);

    @Accessor("blockPosition")
    void horizon$setBlockPosition(net.minecraft.core.BlockPos position);

    @Accessor("chunkPosition")
    void horizon$setChunkPosition(net.minecraft.world.level.ChunkPos position);

    @Accessor("bb")
    void horizon$setBoundingBox(net.minecraft.world.phys.AABB box);

    @Accessor("dimensions")
    void horizon$setDimensions(net.minecraft.world.entity.EntityDimensions dimensions);

    @Accessor("eyeHeight")
    void horizon$setEyeHeight(float height);

    @Accessor("level")
    void horizon$setLevel(net.minecraft.world.level.Level level);

    @Mutable
    @Accessor("fluidOnEyes")
    void horizon$setFluidOnEyes(java.util.Set<net.minecraft.tags.TagKey<net.minecraft.world.level.material.Fluid>> fluids);

    @Accessor("fluidOnEyes")
    java.util.Set<net.minecraft.tags.TagKey<net.minecraft.world.level.material.Fluid>> horizon$fluidOnEyes();
}
