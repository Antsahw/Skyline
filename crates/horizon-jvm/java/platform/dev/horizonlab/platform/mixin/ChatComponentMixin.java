package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A mod's client code writing to its client's chat (ChatComponent.addClientSystemMessage:
 * ReplayMod's "recording started"): the chat is Horizon's window, so the line goes to the
 * player whose client runs now, as their own messages do (LocalPlayerMessagesMixin).
 */
@Mixin(ChatComponent.class)
public abstract class ChatComponentMixin {
    @Inject(method = "addClientSystemMessage", at = @At("HEAD"), cancellable = true)
    private void horizon$chat(Component message, CallbackInfo ci) {
        var minecraft = HorizonClient.minecraft();
        if (HorizonClient.headless() && minecraft != null && minecraft.player != null
                && HorizonClient.clientMessage(minecraft.player, message, false)) {
            ci.cancel();
        }
    }
}
