package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonServer;
import java.util.Set;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import net.minecraft.world.entity.PositionMoveRotation;
import net.minecraft.world.entity.Relative;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A mirrored player's connection: a teleport becomes Horizon's. */
@Mixin(ServerGamePacketListenerImpl.class)
public abstract class GameListenerMixin {
    @Inject(method = "teleport(Lnet/minecraft/world/entity/PositionMoveRotation;Ljava/util/Set;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$teleport(PositionMoveRotation change, Set<Relative> relatives, CallbackInfo ci) {
        ServerGamePacketListenerImpl self = (ServerGamePacketListenerImpl) (Object) this;
        if (!HorizonServer.isMirror(self) || self.player == null) return;
        PositionMoveRotation to = PositionMoveRotation.calculateAbsolute(PositionMoveRotation.of(self.player), change, relatives);
        HorizonServer.teleported(self.player, (ServerLevel) self.player.level(), to);
        ci.cancel();
    }

    /**
     * A mod handing an attack to a mirrored player's connection (a mod's own attack
     * request, as vanilla's attack packet): the player attacks the entity the level has under
     * that id, as handleAttack does past its checks (the mirror has no packet processor: the
     * thread check would throw). Horizon's own attacks don't come this way.
     */
    @Inject(method = "handleAttack", at = @At("HEAD"), cancellable = true)
    private void horizon$attack(net.minecraft.network.protocol.game.ServerboundAttackPacket packet, CallbackInfo ci) {
        ServerGamePacketListenerImpl self = (ServerGamePacketListenerImpl) (Object) this;
        if (!HorizonServer.isMirror(self) || self.player == null) return;
        ci.cancel();
        var target = self.player.level().getEntity(packet.entityId());
        if (target == null || target == self.player || self.player.isSpectator()) return;
        if (target instanceof net.minecraft.world.entity.item.ItemEntity || target instanceof net.minecraft.world.entity.ExperienceOrb) return;
        self.player.attack(target);
    }
}
