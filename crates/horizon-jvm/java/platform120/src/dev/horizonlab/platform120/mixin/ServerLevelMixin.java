package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * What a mirror level can't do without a server and chunks behind it: quiet for now. Sounds,
 * particles and level events are Horizon's to play (a later step sends them); neighbour
 * updates and scheduled ticks are Horizon's to run.
 */
@Mixin(ServerLevel.class)
public abstract class ServerLevelMixin {
    @Inject(method = "playSeededSound", at = @At("HEAD"), cancellable = true)
    private void horizon$sound(CallbackInfo ci) {
        ci.cancel();
    }

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

    @Inject(method = "sendParticles(Lnet/minecraft/core/particles/ParticleOptions;DDDIDDDD)I", at = @At("HEAD"), cancellable = true)
    private void horizon$particles(CallbackInfoReturnable<Integer> cir) {
        cir.setReturnValue(0);
    }

    /**
     * The entities: none, rather than the entity manager a mirror level never made (which every
     * asker would throw on). Placing a block asks, through isUnobstructed.
     */
    @Inject(method = "getEntities()Lnet/minecraft/world/level/entity/LevelEntityGetter;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$entities(
            CallbackInfoReturnable<net.minecraft.world.level.entity.LevelEntityGetter<
                    net.minecraft.world.entity.Entity>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.NoEntities.INSTANCE);
    }
}
