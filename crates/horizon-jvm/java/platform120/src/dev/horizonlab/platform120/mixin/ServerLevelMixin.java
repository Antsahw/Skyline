package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * What a mirror level can't do without a server and chunks behind it. Sounds and particles go
 * to Horizon, which sends them to its players; level events are Horizon's to play (a later
 * step sends them); neighbour updates and scheduled ticks are Horizon's to run.
 */
@Mixin(ServerLevel.class)
public abstract class ServerLevelMixin {
    @Inject(method = "levelEvent", at = @At("HEAD"), cancellable = true)
    private void horizon$levelEvent(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "gameEvent", at = @At("HEAD"), cancellable = true)
    private void horizon$gameEvent(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "sendBlockUpdated", at = @At("HEAD"), cancellable = true)
    private void horizon$blockUpdated(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "updateNeighborsAt", at = @At("HEAD"), cancellable = true)
    private void horizon$neighbours(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "blockUpdated", at = @At("HEAD"), cancellable = true)
    private void horizon$updated(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "neighborChanged", at = @At("HEAD"), cancellable = true)
    private void horizon$neighbourChanged(CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "blockEvent", at = @At("HEAD"), cancellable = true)
    private void horizon$blockEvent(CallbackInfo ci) {
        ci.cancel();
    }

    /**
     * Particles a mod sends its players (ServerLevel.sendParticles: a cooking pot's steam, a
     * mob's hearts): Horizon sends them, as the packet vanilla would.
     */
    @Inject(method = "sendParticles(Lnet/minecraft/core/particles/ParticleOptions;DDDIDDDD)I", at = @At("HEAD"), cancellable = true)
    private void horizon$particles(net.minecraft.core.particles.ParticleOptions options, double x, double y, double z,
            int count, double dx, double dy, double dz, double speed, CallbackInfoReturnable<Integer> cir) {
        dev.horizonlab.platform120.Mirror120.particles((ServerLevel) (Object) this, options, x, y, z, count, dx, dy, dz, speed);
        cir.setReturnValue(0);
    }

    /**
     * What a mod drops or ejects into the world (a chest spilling its items, a machine putting
     * its output out): Horizon spawns it, since it owns this world's entities and a mirror
     * level has nowhere to put one. On ServerLevel and not Level, which has no such method.
     */
    @Inject(method = "addFreshEntity", at = @At("HEAD"), cancellable = true)
    private void horizon$addEntity(net.minecraft.world.entity.Entity entity,
            CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Mirror120.addEntity(
                (net.minecraft.world.level.Level) (Object) this, entity));
    }

    /**
     * The entities: the mods' own living here (Entities120), rather than the entity manager a
     * mirror level never made (which every asker would throw on). Placing a block asks, through
     * isUnobstructed; a mob looking around asks.
     */
    @Inject(method = "getEntities()Lnet/minecraft/world/level/entity/LevelEntityGetter;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$entities(
            CallbackInfoReturnable<net.minecraft.world.level.entity.LevelEntityGetter<
                    net.minecraft.world.entity.Entity>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Entities120.of((net.minecraft.world.level.Level) (Object) this));
    }

    /** A mirror level's players are Horizon's in it this tick (Mirror120.tickEntities). */
    @Inject(method = "players", at = @At("HEAD"), cancellable = true)
    private void horizon$players(CallbackInfoReturnable<java.util.List<net.minecraft.server.level.ServerPlayer>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Mirror120.present((net.minecraft.server.level.ServerLevel) (Object) this));
    }

    /**
     * Raids are Horizon's: a mirror level has none (its raid manager was never made), and an
     * illager of a mod's asking whether it's in one (Raider.aiStep) is told it isn't.
     */
    @Inject(method = "getRaidAt", at = @At("HEAD"), cancellable = true)
    private void horizon$raidAt(net.minecraft.core.BlockPos pos, CallbackInfoReturnable<net.minecraft.world.entity.raid.Raid> cir) {
        cir.setReturnValue(null);
    }

    @Inject(method = "isRaided", at = @At("HEAD"), cancellable = true)
    private void horizon$raided(net.minecraft.core.BlockPos pos, CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(false);
    }

    /**
     * A sound played in a mirror level (a mod's block or mob, as Level.playSound ends here):
     * Horizon plays it for its players, by its name, as the packet vanilla would send them.
     */
    @Inject(method = "playSeededSound(Lnet/minecraft/world/entity/player/Player;DDDLnet/minecraft/core/Holder;Lnet/minecraft/sounds/SoundSource;FFJ)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$sound(net.minecraft.world.entity.player.Player except, double x, double y, double z,
            net.minecraft.core.Holder<net.minecraft.sounds.SoundEvent> sound, net.minecraft.sounds.SoundSource source,
            float volume, float pitch, long seed, CallbackInfo ci) {
        dev.horizonlab.platform120.Mirror120.sound((ServerLevel) (Object) this, x, y, z, sound, source, volume, pitch);
        ci.cancel();
    }

    @Inject(method = "playSeededSound(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/entity/Entity;Lnet/minecraft/core/Holder;Lnet/minecraft/sounds/SoundSource;FFJ)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$entitySound(net.minecraft.world.entity.player.Player except, net.minecraft.world.entity.Entity entity,
            net.minecraft.core.Holder<net.minecraft.sounds.SoundEvent> sound, net.minecraft.sounds.SoundSource source,
            float volume, float pitch, long seed, CallbackInfo ci) {
        dev.horizonlab.platform120.Mirror120.sound((ServerLevel) (Object) this, entity.getX(), entity.getY(), entity.getZ(),
                sound, source, volume, pitch);
        ci.cancel();
    }
}
