package dev.horizonlab.platform120.mixin;

import dev.horizonlab.platform120.Graphics120;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.renderer.RenderType;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A mod's screen drawn into {@link Graphics120} instead of onto a GPU this runtime does not
 * have. Each call is recorded and cancelled.
 *
 * <p>The targets are the methods the others funnel into, so an overload is caught once rather
 * than six times: {@code fill} and {@code fillGradient} end at the ones taking a RenderType,
 * every {@code blit} ends at {@code innerBlit}, and every {@code renderItem} at the one taking
 * a living entity and a level. Text is the exception -- its three forms reach the font by three
 * routes -- so all three are taken, the styled one first, which stops it reaching the shaped
 * one and being recorded twice.
 *
 * <p>Nothing happens unless a screen is being recorded: with no recorder these are not a
 * mod's drawing but vanilla's own, and there is nothing to take.
 */
@Mixin(GuiGraphics.class)
public abstract class GuiGraphicsMixin {
    @Inject(method = "fill(Lnet/minecraft/client/renderer/RenderType;IIIIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$fill(RenderType type, int x0, int y0, int x1, int y1, int z, int color,
            CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.fill(x0, y0, x1, y1, color);
            ci.cancel();
        }
    }

    @Inject(method = "fillGradient(Lnet/minecraft/client/renderer/RenderType;IIIIIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$fillGradient(RenderType type, int x0, int y0, int x1, int y1, int z,
            int top, int bottom, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.gradient(x0, y0, x1, y1, top, bottom);
            ci.cancel();
        }
    }

    /**
     * Every blit ends here. 1.20.1 gives the corners already as fractions of the sheet and the
     * horizontal pair before the vertical, which is not the order anything else uses.
     */
    @Inject(method = "innerBlit(Lnet/minecraft/resources/ResourceLocation;IIIIIFFFF)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$blit(ResourceLocation texture, int x0, int x1, int y0, int y1, int z,
            float u0, float u1, float v0, float v1, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.blitUv(texture, x0, y0, x1 - x0, y1 - y0, u0, v0, u1, v1);
            ci.cancel();
        }
    }

    @Inject(method = "renderItem(Lnet/minecraft/world/entity/LivingEntity;"
            + "Lnet/minecraft/world/level/Level;Lnet/minecraft/world/item/ItemStack;IIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$item(net.minecraft.world.entity.LivingEntity entity,
            net.minecraft.world.level.Level level, ItemStack stack, int x, int y, int seed,
            int guiOffset, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.item(stack, x, y);
            ci.cancel();
        }
    }

    /** Styled text: taken before it is shaped, so its styling survives as the component's JSON. */
    @Inject(method = "drawString(Lnet/minecraft/client/gui/Font;"
            + "Lnet/minecraft/network/chat/Component;IIIZ)I", at = @At("HEAD"), cancellable = true)
    private void horizon$textComponent(Font font, Component text, int x, int y, int color,
            boolean shadow, CallbackInfoReturnable<Integer> cir) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.text(text, x, y, color, shadow);
            cir.setReturnValue(font == null ? 0 : font.width(text));
        }
    }

    /** Text already shaped by someone else: its styling is gone, so it goes as characters. */
    @Inject(method = "drawString(Lnet/minecraft/client/gui/Font;"
            + "Lnet/minecraft/util/FormattedCharSequence;IIIZ)I", at = @At("HEAD"),
            cancellable = true)
    private void horizon$textShaped(Font font, FormattedCharSequence text, int x, int y,
            int color, boolean shadow, CallbackInfoReturnable<Integer> cir) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.text(text, x, y, color, shadow);
            cir.setReturnValue(font == null ? 0 : font.width(text));
        }
    }

    @Inject(method = "drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;IIIZ)I",
            at = @At("HEAD"), cancellable = true)
    private void horizon$textPlain(Font font, String text, int x, int y, int color,
            boolean shadow, CallbackInfoReturnable<Integer> cir) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.text(text, x, y, color, shadow);
            cir.setReturnValue(font == null || text == null ? 0 : font.width(text));
        }
    }

    @Inject(method = "enableScissor(IIII)V", at = @At("HEAD"), cancellable = true)
    private void horizon$scissor(int x0, int y0, int x1, int y1, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.scissor(x0, y0, x1, y1);
            ci.cancel();
        }
    }

    @Inject(method = "disableScissor()V", at = @At("HEAD"), cancellable = true)
    private void horizon$unscissor(CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            to.unscissor();
            ci.cancel();
        }
    }
}
