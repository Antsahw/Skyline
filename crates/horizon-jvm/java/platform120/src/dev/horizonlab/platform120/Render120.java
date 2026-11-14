package dev.horizonlab.platform120;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.RenderType;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.Level;

/**
 * The 1.20.1 mods' entities as their own renderers draw them (phase 6d.2). Each mod's renderer
 * is made as the client makes it (its provider, with the client's baked model layers: Fabric's
 * registry put the mod's own layers among vanilla's), and each tick draws its entity, at the
 * origin, into a capture: what it wrote, quad by quad, with the texture of the render type it
 * wrote into. Horizon draws those quads where the entity is.
 */
final class Render120 {
    private Render120() {}

    private static Map<EntityType<?>, EntityRenderer<?>> renderers;
    /** The mods' block entity renderers, by their type. */
    private static Map<net.minecraft.world.level.block.entity.BlockEntityType<?>,
            net.minecraft.client.renderer.blockentity.BlockEntityRenderer<?>> blockRenderers;
    private static net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher blockDispatcher;
    private static net.minecraft.client.Camera camera;
    private static final Set<String> FAILED = new HashSet<>();
    /** A texture's name in a render type's description: `ns:textures/....png`. */
    private static final Pattern TEXTURE = Pattern.compile("([a-z0-9_.-]+:[a-z0-9_./-]+\\.png)");

    /** The mods' renderers, made once (vanilla's types are Horizon's to draw). */
    private static synchronized void setUp() {
        if (renderers != null) return;
        renderers = new HashMap<>();
        var client = net.minecraft.client.Minecraft.getInstance();
        // The renderers' font (a sign's text measured): the screens' measuring one.
        Screens120.font();
        var resources = Models120.clientResources();
        var models = new net.minecraft.client.model.geom.EntityModelSet();
        models.onResourceManagerReload(resources);
        // Some renderers ask the client for them as they're made (Deeper and Darker's skull).
        if (client.getEntityModels() == null) {
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setEntityModels(models);
        }
        var dispatcher = new EntityRenderDispatcher(client, null, null, null, null, client.options, models);
        dispatcher.camera = new net.minecraft.client.Camera();
        // Blocks a renderer draws (a moobloom's flowers on its back) through the client's baked
        // block models (Models120), on the stand-in sprites: split into sprites below.
        var shaper = new net.minecraft.client.renderer.block.BlockModelShaper(null);
        shaper.replaceCache(Models120.baked());
        var blocks = new net.minecraft.client.renderer.block.BlockRenderDispatcher(shaper, null, client.getBlockColors());
        // Some renderers ask the client for it (Friends & Foes' flower layer), as they're made.
        if (client.getBlockRenderer() == null) {
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setBlockRenderer(blocks);
        }
        var context = new EntityRendererProvider.Context(dispatcher, null, blocks,
                dispatcher.getItemInHandRenderer(), resources, models, null);
        for (var entry : dev.horizonlab.platform120.mixin.EntityRenderersAccess.horizon$providers().entrySet()) {
            var id = BuiltInRegistries.ENTITY_TYPE.getKey(entry.getKey());
            if ("minecraft".equals(id.getNamespace())) continue;
            try {
                renderers.put(entry.getKey(), entry.getValue().create(context));
            } catch (RuntimeException | LinkageError e) {
                Mirror120.log("the renderer of " + id + " can't be made: " + e);
            }
        }
        Mirror120.log(renderers.size() + " mod entity renderer(s) made");

        // Block entity renderers (phase 7b): a cutting board's item, a skillet's food. They draw
        // items through an ItemRenderer over the client's baked item models (Models120).
        var modelManager = Allocate.make(net.minecraft.client.resources.model.ModelManager.class);
        var topLevel = Models120.topLevel();
        ((dev.horizonlab.platform120.mixin.ModelManagerAccess) modelManager).horizon$setBakedRegistry(topLevel);
        ((dev.horizonlab.platform120.mixin.ModelManagerAccess) modelManager).horizon$setMissingModel(
                topLevel.get(net.minecraft.client.resources.model.ModelBakery.MISSING_MODEL_LOCATION));
        var itemColors = net.minecraft.client.color.item.ItemColors.createDefault(client.getBlockColors());
        var holder = new net.minecraft.client.renderer.entity.ItemRenderer[1];
        blockDispatcher = new net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher(
                client.font, models, () -> blocks, () -> holder[0], () -> dispatcher);
        var withoutLevel = new net.minecraft.client.renderer.BlockEntityWithoutLevelRenderer(blockDispatcher, models);
        holder[0] = new net.minecraft.client.renderer.entity.ItemRenderer(client, null, modelManager, itemColors, withoutLevel);
        holder[0].getItemModelShaper().rebuildCache();
        // Renderers ask the client for it as they draw (Farmer's Delight's cutting board).
        if (client.getItemRenderer() == null) {
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setItemRenderer(holder[0]);
        }
        camera = dispatcher.camera;
        // Models that look at the player ask the client's game renderer for its camera as they
        // pose (Naturalist's crab): a renderer made without its constructor, with this camera.
        if (client.gameRenderer == null) {
            var renderer = Allocate.make(net.minecraft.client.renderer.GameRenderer.class);
            ((dev.horizonlab.platform120.mixin.GameRendererAccess) renderer).horizon$setMainCamera(camera);
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setGameRenderer(renderer);
        }
        var blockContext = new net.minecraft.client.renderer.blockentity.BlockEntityRendererProvider.Context(
                blockDispatcher, blocks, holder[0], dispatcher, models, client.font);
        blockRenderers = new HashMap<>();
        for (var entry : dev.horizonlab.platform120.mixin.BlockEntityRenderersAccess.horizon$providers().entrySet()) {
            var id = BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(entry.getKey());
            if (id == null || "minecraft".equals(id.getNamespace())) continue;
            try {
                blockRenderers.put(entry.getKey(), entry.getValue().create(blockContext));
            } catch (RuntimeException | LinkageError e) {
                Mirror120.log("the renderer of " + id + " can't be made: " + e);
            }
        }
        Mirror120.log(blockRenderers.size() + " mod block entity renderer(s) made");
    }

    /**
     * What the mods' block entities at `positions` (x, y, z each) look like, as their own
     * renderers draw them: JSON {"x,y,z": [{"tex", "o", "v"}...]}, positions relative to the
     * block's corner; those with no renderer (or nothing drawn) left out.
     */
    static String renderBlockEntities(Level level, int[] positions) {
        setUp();
        JsonObject out = new JsonObject();
        if (blockRenderers.isEmpty()) return out.toString();
        blockDispatcher.prepare(level, camera, null);
        for (int i = 0; i + 2 < positions.length; i += 3) {
            var pos = new net.minecraft.core.BlockPos(positions[i], positions[i + 1], positions[i + 2]);
            var entity = level.getBlockEntity(pos);
            if (entity == null) continue;
            @SuppressWarnings("unchecked")
            var renderer = (net.minecraft.client.renderer.blockentity.BlockEntityRenderer<net.minecraft.world.level.block.entity.BlockEntity>)
                    blockRenderers.get(entity.getType());
            if (renderer == null) continue;
            Capture capture = new Capture();
            try {
                renderer.render(entity, 1.0f, new PoseStack(), capture, 0xF000F0,
                        net.minecraft.client.renderer.texture.OverlayTexture.NO_OVERLAY);
            } catch (RuntimeException | LinkageError e) {
                String type = String.valueOf(BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(entity.getType()));
                if (FAILED.add(type)) {
                    Mirror120.log("a " + type + " didn't draw: " + e);
                    for (StackTraceElement at : e.getStackTrace()) Mirror120.log("    at " + at);
                }
                continue;
            }
            JsonArray meshes = capture.meshes();
            if (!meshes.isEmpty()) out.add(pos.getX() + "," + pos.getY() + "," + pos.getZ(), meshes);
        }
        return out.toString();
    }

    /**
     * What a level's mod entities look like this tick, as JSON: {id: [{"tex", "o", "v"}...]},
     * each mesh's vertices nine numbers each (x y z relative to the entity, u v in the texture,
     * the normal, ARGB), "o" its packed overlay (the hurt flash).
     */
    static String render(Level level) {
        setUp();
        JsonObject out = new JsonObject();
        for (Entity entity : Entities120.of(level).getAll()) {
            if (entity.isRemoved()) continue;
            @SuppressWarnings("unchecked")
            EntityRenderer<Entity> renderer = (EntityRenderer<Entity>) renderers.get(entity.getType());
            if (renderer == null) continue;
            Capture capture = new Capture();
            try {
                renderer.render(entity, entity.getYRot(), 1.0f, new PoseStack(), capture, 0xF000F0);
            } catch (RuntimeException | LinkageError e) {
                String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
                if (FAILED.add(type)) {
                    Mirror120.log("a " + type + " didn't draw: " + e);
                    for (StackTraceElement at : e.getStackTrace()) Mirror120.log("    at " + at);
                }
            }
            out.add(Integer.toString(entity.getId()), capture.meshes());
        }
        return out.toString();
    }

    /** The buffers a renderer writes into: one mesh a texture. */
    private static final class Capture implements MultiBufferSource {
        final Map<String, Mesh> meshes = new LinkedHashMap<>();

        @Override
        public VertexConsumer getBuffer(RenderType type) {
            Matcher m = TEXTURE.matcher(type.toString());
            String texture = m.find() ? m.group(1) : "minecraft:textures/misc/white.png";
            return meshes.computeIfAbsent(texture, t -> new Mesh());
        }

        JsonArray meshes() {
            // Quads written on an atlas (a block drawn on a mob) go each to its own sprite's
            // mesh: the stand-in sprites are slots of U (Sprites120), so a quad's U says which.
            Map<String, Mesh> split = new LinkedHashMap<>();
            for (var e : meshes.entrySet()) {
                if (!e.getKey().contains(":textures/atlas/")) {
                    split.merge(e.getKey(), e.getValue(), (a, b) -> a);
                    continue;
                }
                List<Double> v = e.getValue().v;
                for (int q = 0; q + 36 <= v.size(); q += 36) {
                    double us = 0;
                    for (int c = 0; c < 4; c++) us += v.get(q + c * 9 + 3);
                    int slot = (int) Math.floor(us / 4);
                    var sprite = Sprites120.slot(slot);
                    if (sprite == null) continue;
                    String texture = sprite.getNamespace() + ":textures/" + sprite.getPath() + ".png";
                    Mesh mesh = split.computeIfAbsent(texture, t -> new Mesh());
                    mesh.overlay = e.getValue().overlay;
                    for (int i = 0; i < 36; i++) mesh.v.add(i % 9 == 3 ? v.get(q + i) - slot : v.get(q + i));
                }
            }
            JsonArray out = new JsonArray();
            for (var e : split.entrySet()) {
                if (e.getValue().v.isEmpty()) continue;
                JsonObject mesh = new JsonObject();
                mesh.addProperty("tex", e.getKey());
                mesh.addProperty("o", e.getValue().overlay);
                JsonArray v = new JsonArray();
                for (double d : e.getValue().v) v.add((float) d);
                mesh.add("v", v);
                out.add(mesh);
            }
            return out;
        }
    }

    /** One texture's vertices, as written. */
    private static final class Mesh implements VertexConsumer {
        final List<Double> v = new ArrayList<>();
        double x, y, z;
        float u, vv, nx, ny, nz;
        int color = -1;
        int overlay;
        boolean fixed;
        int fixedColor = -1;

        @Override public VertexConsumer vertex(double x, double y, double z) { this.x = x; this.y = y; this.z = z; return this; }
        @Override public VertexConsumer color(int r, int g, int b, int a) { color = a << 24 | r << 16 | g << 8 | b; return this; }
        @Override public VertexConsumer uv(float u, float v) { this.u = u; this.vv = v; return this; }
        @Override public VertexConsumer overlayCoords(int u, int v) { overlay = v << 16 | u; return this; }
        @Override public VertexConsumer uv2(int u, int v) { return this; }
        @Override public VertexConsumer normal(float x, float y, float z) { nx = x; ny = y; nz = z; return this; }
        @Override public void defaultColor(int r, int g, int b, int a) { fixed = true; fixedColor = a << 24 | r << 16 | g << 8 | b; }
        @Override public void unsetDefaultColor() { fixed = false; }

        @Override public void endVertex() {
            v.add(x); v.add(y); v.add(z); v.add((double) u); v.add((double) vv);
            v.add((double) nx); v.add((double) ny); v.add((double) nz);
            v.add((double) Integer.toUnsignedLong(fixed ? fixedColor : color));
            color = -1;
        }
    }
}
