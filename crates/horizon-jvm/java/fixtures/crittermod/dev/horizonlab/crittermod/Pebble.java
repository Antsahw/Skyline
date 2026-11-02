package dev.horizonlab.crittermod;

import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.projectile.ThrowableProjectile;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;

/** A mod's own projectile (no vanilla class but ThrowableProjectile): 2 damage on what it hits, then gone. */
public class Pebble extends ThrowableProjectile {
    public static final float DAMAGE = 2.0F;

    public Pebble(EntityType<? extends Pebble> type, Level level) {
        super(type, level);
    }

    @Override
    protected void defineSynchedData(SynchedEntityData.Builder builder) {
    }

    @Override
    protected void onHitEntity(EntityHitResult hit) {
        super.onHitEntity(hit);
        if (level() instanceof ServerLevel level) {
            hit.getEntity().hurtServer(level, damageSources().thrown(this, getOwner()), DAMAGE);
        }
    }

    @Override
    protected void onHit(HitResult hit) {
        super.onHit(hit);
        if (!level().isClientSide()) discard();
    }
}
