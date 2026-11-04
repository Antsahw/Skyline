package dev.horizonlab.kineticmod;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.math.Axis;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.rendering.v1.BlockEntityRendererRegistry;
import net.minecraft.client.Minecraft;
import net.minecraft.client.color.block.BlockColors;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderer;
import net.minecraft.client.renderer.blockentity.state.BlockEntityRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.resources.Identifier;
import net.minecraft.world.phys.Vec3;

/**
 * Create's client, in small: its renderer's model helper exists only once the level renderer
 * reloaded (Flywheel's ModelRenderHelper.onReloadLevelRenderer, from LevelExtractor.allChanged),
 * the resource reload's end is marked (Minecraft.onResourceLoadFinished), and its client tick
 * reads the player's first-person hands and its own data on the player, keyed by class.
 */
public final class KineticClient implements ClientModInitializer {
    static final Identifier TEXTURE = Identifier.withDefaultNamespace("textures/block/stripped_oak_log.png");
    /** Set at the level renderer's reload (ReloadMixin), as Flywheel's helper is. */
    public static BlockColors helper;
    public static boolean reloadEnded;
    private static boolean told;

    @Override
    public void onInitializeClient() {
        BlockEntityRendererRegistry.register(KineticMod.SHAFT_ENTITY, context -> new ShaftRenderer());
        ClientTickEvents.END_CLIENT_TICK.register(mc -> {
            var player = mc.player;
            if (player == null || mc.level == null) return;
            // Create's ExtendoGripRenderHandler: the held item as the first-person hands have it.
            var held = player.firstPersonHandsAndItems().getClass();
            // Create's AllSynchedDatas: an accessor per entity class, made by the class's Builder.
            if (!KineticMod.DATA_CLASSES.contains(player.getClass())) {
                throw new NullPointerException("no synced data accessor for " + player.getClass().getSimpleName());
            }
            if (!told && helper != null) {
                told = true;
                String text = "horizon_kinetic: client ready (hands " + held.getSimpleName() + ", data for "
                        + player.getClass().getSimpleName() + ", reload ended " + reloadEnded + ")";
                player.sendSystemMessage(net.minecraft.network.chat.Component.literal(text));
                System.out.println(text);
            }
        });
    }

    static void box(PoseStack.Pose pose, VertexConsumer out, float h, int argb, int light) {
        float a = -h, b = h;
        float[][] faces = {
            {a, a, b, b, a, b, b, b, b, a, b, b, 0, 0, 1},
            {b, a, a, a, a, a, a, b, a, b, b, a, 0, 0, -1},
            {a, a, a, a, a, b, a, b, b, a, b, a, -1, 0, 0},
            {b, a, b, b, a, a, b, b, a, b, b, b, 1, 0, 0},
            {a, b, b, b, b, b, b, b, a, a, b, a, 0, 1, 0},
            {a, a, a, b, a, a, b, a, b, a, a, b, 0, -1, 0},
        };
        float[][] uvs = {{0, 1}, {1, 1}, {1, 0}, {0, 0}};
        for (float[] f : faces) {
            for (int i = 0; i < 4; i++) {
                out.addVertex(pose, f[i * 3], f[i * 3 + 1], f[i * 3 + 2]).setColor(argb).setUv(uvs[i][0], uvs[i][1])
                        .setOverlay(OverlayTexture.NO_OVERLAY).setLight(light).setNormal(pose, f[12], f[13], f[14]);
            }
        }
    }

    /** The shaft turned by its block entity's angle, above its block. Throws until the helper is there. */
    static final class ShaftRenderer implements BlockEntityRenderer<KineticMod.ShaftBlockEntity, ShaftRenderer.State> {
        static final class State extends BlockEntityRenderState {
            float angle;
        }

        @Override
        public State createRenderState() {
            return new State();
        }

        @Override
        public void extractRenderState(KineticMod.ShaftBlockEntity be, State state, float partialTick, Vec3 camera,
                                       ModelFeatureRenderer.CrumblingOverlay breaking) {
            BlockEntityRenderer.super.extractRenderState(be, state, partialTick, camera, breaking);
            helper.getClass(); // Create: ModelRenderHelper.INSTANCE.updateOutput(...), null before the reload
            state.angle = be.angle;
        }

        @Override
        public void submit(State state, PoseStack pose, SubmitNodeCollector collector, CameraRenderState camera) {
            pose.pushPose();
            pose.translate(0.5F, 1.5F, 0.5F);
            pose.rotateDegrees(Axis.YP, state.angle);
            int light = state.lightCoords;
            collector.submitCustomGeometry(pose, RenderTypes.entityCutout(TEXTURE), (p, out) -> {
                box(p, out, 0.15F, 0xFFFFFFFF, light);
                p.translate(0.3F, 0, 0);
                box(p, out, 0.12F, 0xFFFF8040, light);
            });
            pose.popPose();
        }
    }
}
