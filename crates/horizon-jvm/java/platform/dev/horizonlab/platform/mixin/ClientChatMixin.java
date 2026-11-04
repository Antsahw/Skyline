package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A line a mod's client code adds to the client's chat itself (ChatComponent
 * .addClientSystemMessage: Simple Voice Chat's "Press V to set up"): the chat is Horizon's
 * window's, so it goes to the game of the player whose client runs now, as their own messages do.
 */
@Mixin(ChatComponent.class)
public abstract class ClientChatMixin {
    @Inject(method = "addClientSystemMessage", at = @At("HEAD"), cancellable = true)
    private void horizon$chat(Component message, CallbackInfo ci) {
        if (!HorizonClient.headless()) return;
        var player = Minecraft.getInstance().player;
        if (player != null && HorizonClient.clientMessage(player, message, false)) ci.cancel();
    }
}
