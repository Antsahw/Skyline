package dev.horizonlab.platform120.mixin;

import com.mojang.blaze3d.platform.GlStateManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * No OpenGL in the headless client. Every GL call 1.20.1 makes goes through GlStateManager, and
 * this runtime has no GL context -- LWJGL would throw on the first one -- so each of its
 * void calls is stopped at the head. A mod's screen sets blending, depth testing and textures
 * around its drawing; the drawing itself is recorded (GuiGraphicsMixin) and drawn by Horizon,
 * which sets its own.
 *
 * <p>The list is every void `_` method of 1.20.1's GlStateManager, taken from Mojang's mappings
 * rather than written by hand. The ones that answer a value (`_genTexture`, `_getInteger`) are
 * left: nothing a screen does asks them.
 */
@Mixin(GlStateManager.class)
public abstract class GlStateManagerMixin {
    @Inject(method = {
            "_activeTexture(I)V",
            "_bindTexture(I)V",
            "_blendEquation(I)V",
            "_blendFuncSeparate(IIII)V",
            "_blendFunc(II)V",
            "_clearColor(FFFF)V",
            "_clearDepth(D)V",
            "_clearStencil(I)V",
            "_clear(IZ)V",
            "_colorMask(ZZZZ)V",
            "_deleteTextures([I)V",
            "_deleteTexture(I)V",
            "_depthFunc(I)V",
            "_depthMask(Z)V",
            "_disableBlend()V",
            "_disableColorLogicOp()V",
            "_disableCull()V",
            "_disableDepthTest()V",
            "_disablePolygonOffset()V",
            "_disableVertexAttribArray(I)V",
            "_drawElements(IIIJ)V",
            "_enableBlend()V",
            "_enableColorLogicOp()V",
            "_enableCull()V",
            "_enableDepthTest()V",
            "_enablePolygonOffset()V",
            "_enableScissorTest()V",
            "_enableVertexAttribArray(I)V",
            "_genTextures([I)V",
            "_getTexImage(IIIIJ)V",
            "_glBindAttribLocation(IILjava/lang/CharSequence;)V",
            "_glBindBuffer(II)V",
            "_glBindFramebuffer(II)V",
            "_glBindRenderbuffer(II)V",
            "_glBindVertexArray(I)V",
            "_glBlitFrameBuffer(IIIIIIIIII)V",
            "_glBufferData(IJI)V",
            "_glBufferData(ILjava/nio/ByteBuffer;I)V",
            "_glCopyTexSubImage2D(IIIIIIII)V",
            "_glDeleteBuffers(I)V",
            "_glDeleteFramebuffers(I)V",
            "_glDeleteRenderbuffers(I)V",
            "_glDeleteVertexArrays(I)V",
            "_glDrawPixels(IIIIJ)V",
            "_glFramebufferRenderbuffer(IIII)V",
            "_glFramebufferTexture2D(IIIII)V",
            "_glRenderbufferStorage(IIII)V",
            "_glUniform1i(II)V",
            "_glUniform1(ILjava/nio/FloatBuffer;)V",
            "_glUniform1(ILjava/nio/IntBuffer;)V",
            "_glUniform2(ILjava/nio/FloatBuffer;)V",
            "_glUniform2(ILjava/nio/IntBuffer;)V",
            "_glUniform3(ILjava/nio/FloatBuffer;)V",
            "_glUniform3(ILjava/nio/IntBuffer;)V",
            "_glUniform4(ILjava/nio/FloatBuffer;)V",
            "_glUniform4(ILjava/nio/IntBuffer;)V",
            "_glUniformMatrix2(IZLjava/nio/FloatBuffer;)V",
            "_glUniformMatrix3(IZLjava/nio/FloatBuffer;)V",
            "_glUniformMatrix4(IZLjava/nio/FloatBuffer;)V",
            "_glUnmapBuffer(I)V",
            "_glUseProgram(I)V",
            "_logicOp(I)V",
            "_pixelStore(II)V",
            "_polygonMode(II)V",
            "_polygonOffset(FF)V",
            "_readPixels(IIIIIIJ)V",
            "_readPixels(IIIIIILjava/nio/ByteBuffer;)V",
            "_stencilFunc(III)V",
            "_stencilMask(I)V",
            "_stencilOp(III)V",
            "_texImage2D(IIIIIIIILjava/nio/IntBuffer;)V",
            "_texParameter(IIF)V",
            "_texParameter(III)V",
            "_texSubImage2D(IIIIIIIIJ)V",
            "_upload(IIIIILcom/mojang/blaze3d/platform/NativeImage$Format;Ljava/nio/IntBuffer;Ljava/util/function/Consumer;)V",
            "_vertexAttribIPointer(IIIIJ)V",
            "_vertexAttribPointer(IIIZIJ)V",
            "_viewport(IIII)V"
    }, at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$noGl(CallbackInfo ci) {
        ci.cancel();
    }

    /**
     * Clipping set straight on GL (Cloth Config's ScissorsHandler, RenderSystem.enableScissor):
     * window pixels from the bottom left. Recorded into the frame being drawn, if any.
     */
    @Inject(method = "_scissorBox(IIII)V", at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$scissorBox(int x, int y, int width, int height, CallbackInfo ci) {
        dev.horizonlab.platform120.Graphics120 to = dev.horizonlab.platform120.Graphics120.current();
        if (to != null) {
            to.glScissor(x, y, width, height);
        }
        ci.cancel();
    }

    @Inject(method = "_disableScissorTest()V", at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$scissorOff(CallbackInfo ci) {
        dev.horizonlab.platform120.Graphics120 to = dev.horizonlab.platform120.Graphics120.current();
        if (to != null) {
            to.glUnscissor();
        }
        ci.cancel();
    }
}
