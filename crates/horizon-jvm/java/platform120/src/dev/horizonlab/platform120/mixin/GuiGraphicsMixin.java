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
    /**
     * Where (x, y) lands under the pose the screen drew it with: a container screen
     * translates to its own corner before drawing its slots and labels, so the coordinates a
     * call is given are not where it goes. Rotation and shear are left out -- screens use
     * translation and scale.
     */
    private int[] horizon$at(float x, float y) {
        GuiGraphics self = (GuiGraphics) (Object) this;
        if (self.pose() == null) {
            return new int[] {Math.round(x), Math.round(y)};
        }
        org.joml.Matrix4f m = self.pose().last().pose();
        return new int[] {
                Math.round(m.m00() * x + m.m10() * y + m.m30()),
                Math.round(m.m01() * x + m.m11() * y + m.m31())};
    }
    @Inject(method = "fill(Lnet/minecraft/client/renderer/RenderType;IIIIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$fill(RenderType type, int x0, int y0, int x1, int y1, int z, int color,
            CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            int[] a = horizon$at(x0, y0), b = horizon$at(x1, y1);
            to.fill(a[0], a[1], b[0], b[1], color);
            ci.cancel();
        }
    }

    /**
     * The RenderType overload takes z *last* -- `(type, x0, y0, x1, y1, from, to, z)` -- unlike
     * `fill`, whose z comes before its colour. Reading it fill's way shifts both colours by one.
     */
    @Inject(method = "fillGradient(Lnet/minecraft/client/renderer/RenderType;IIIIIII)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$fillGradient(RenderType type, int x0, int y0, int x1, int y1, int top,
            int bottom, int z, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            int[] a = horizon$at(x0, y0), b = horizon$at(x1, y1);
            to.gradient(a[0], a[1], b[0], b[1], top, bottom);
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
            int[] a = horizon$at(x0, y0), b = horizon$at(x1, y1);
            to.blitUv(texture, a[0], a[1], b[0] - a[0], b[1] - a[1], u0, v0, u1, v1);
            ci.cancel();
        }
    }

    /**
     * A sprite of an atlas (an empty slot's outline): recorded as its texture's name, which
     * Horizon finds in its own atlas. Stops before vanilla asks the stand-in for its corners.
     */
    @Inject(method = "blit(IIIIILnet/minecraft/client/renderer/texture/TextureAtlasSprite;)V",
            at = @At("HEAD"), cancellable = true)
    private void horizon$sprite(int x, int y, int z, int width, int height,
            net.minecraft.client.renderer.texture.TextureAtlasSprite sprite, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            String texture = dev.horizonlab.platform120.Sprites120.texture(sprite);
            if (texture != null) {
                int[] a = horizon$at(x, y), b = horizon$at(x + width, y + height);
                to.blitUv(ResourceLocation.tryParse(texture), a[0], a[1], b[0] - a[0],
                        b[1] - a[1], 0f, 0f, 1f, 1f);
            }
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
            int[] a = horizon$at(x, y);
            to.item(stack, a[0], a[1]);
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
            int[] a = horizon$at(x, y);
            to.text(text, a[0], a[1], color, shadow);
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
            int[] a = horizon$at(x, y);
            to.text(text, a[0], a[1], color, shadow);
            cir.setReturnValue(font == null ? 0 : font.width(text));
        }
    }

    @Inject(method = "drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;IIIZ)I",
            at = @At("HEAD"), cancellable = true)
    private void horizon$textPlain(Font font, String text, int x, int y, int color,
            boolean shadow, CallbackInfoReturnable<Integer> cir) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            int[] a = horizon$at(x, y);
            to.text(text, a[0], a[1], color, shadow);
            cir.setReturnValue(font == null || text == null ? 0 : font.width(text));
        }
    }

    @Inject(method = "enableScissor(IIII)V", at = @At("HEAD"), cancellable = true)
    private void horizon$scissor(int x0, int y0, int x1, int y1, CallbackInfo ci) {
        Graphics120 to = Graphics120.current();
        if (to != null) {
            int[] a = horizon$at(x0, y0), b = horizon$at(x1, y1);
            to.scissor(a[0], a[1], b[0], b[1]);
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
