package dev.horizonlab.platform120.mixin;

import java.util.function.Function;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The headless client's texture atlases (names only, for Horizon to draw from its own) and window. */
@Mixin(Minecraft.class)
public abstract class MinecraftMixin {
    @Inject(method = "getTextureAtlas(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/function/Function;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$atlas(ResourceLocation atlas,
            CallbackInfoReturnable<Function<ResourceLocation, TextureAtlasSprite>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Sprites120.atlas(atlas));
    }

    /**
     * The window, which a container screen asks for only to ask GLFW whether shift is held:
     * an allocated one, the question answered by InputConstantsMixin.
     */
    @Inject(method = "getWindow()Lcom/mojang/blaze3d/platform/Window;", at = @At("HEAD"), cancellable = true)
    private void horizon$window(CallbackInfoReturnable<com.mojang.blaze3d.platform.Window> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Screens120.window());
    }

    /**
     * A screen the client half opens itself (Minecraft.setScreen), usually from a packet: a
     * container's (Porting Lib's open_screen) is the screen Screens120 already makes from the
     * server's menu and shows on Horizon, so this one is not shown twice; another kind (a
     * mod's own page, a config screen) is not shown on Horizon yet. Either way the headless
     * client has no window, mouse or sound to hand over to it, which vanilla's setScreen does.
     */
    @Inject(method = "setScreen(Lnet/minecraft/client/gui/screens/Screen;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$noScreen(net.minecraft.client.gui.screens.Screen screen, CallbackInfo ci) {
        dev.horizonlab.platform120.Screens120.clientOpened(screen);
        ci.cancel();
    }

    /**
     * Whether the account's text filtering is on (a book's screen, a sign's, ask): the headless
     * client has no Mojang account services behind it, so off, as an offline client's is.
     * Horizon shows the text; what a player may write is the server's to judge.
     */
    @Inject(method = "isTextFilteringEnabled()Z", at = @At("HEAD"), cancellable = true)
    private void horizon$noFilter(CallbackInfoReturnable<Boolean> cir) {
        cir.setReturnValue(false);
    }
}
