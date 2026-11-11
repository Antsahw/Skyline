package dev.horizonlab.platform120.mixin;

import dev.horizonlab.platform120.Screens120;
import java.util.OptionalInt;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.MenuProvider;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mod opening a menu for a mirror player. Vanilla's own openMenu runs untouched -- the menu
 * is made, its slots listened to, the player's containerMenu set -- and only the packet that
 * would tell a client to show it goes nowhere (ServerGamePacketListenerMixin). What that
 * packet carried, the title, is kept here instead, since the menu itself does not hold it.
 */
@Mixin(ServerPlayer.class)
public abstract class ServerPlayerMenuMixin {
    @Inject(method = "openMenu(Lnet/minecraft/world/MenuProvider;)Ljava/util/OptionalInt;",
            at = @At("RETURN"))
    private void horizon$opened(MenuProvider provider, CallbackInfoReturnable<OptionalInt> cir) {
        if (provider != null && cir.getReturnValue() != null && cir.getReturnValue().isPresent()) {
            Screens120.opened((ServerPlayer) (Object) this, provider.getDisplayName());
        }
    }
}
