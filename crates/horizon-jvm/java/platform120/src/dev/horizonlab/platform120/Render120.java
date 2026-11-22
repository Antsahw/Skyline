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
    /** The client's item renderer, over its baked item models (made in setUp). */
    private static net.minecraft.client.renderer.entity.ItemRenderer items;
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
        var resources = Loader120.get().clientResources();
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
        shaper.replaceCache(Loader120.get().baked());
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
        Loader120.get().featureRenderers(context);

        // Block entity renderers (phase 7b): a cutting board's item, a skillet's food. They draw
        // items through an ItemRenderer over the client's baked item models (Models120).
        var modelManager = Allocate.make(net.minecraft.client.resources.model.ModelManager.class);
        var topLevel = Loader120.get().topLevel();
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
        items = holder[0];
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
     * The mods' items whose model is their code (a model with parent `builtin/entity`: Epic
     * Knights' shields, a mod's chest as an item), each drawn once by its own renderer (the
     * item's `BlockEntityWithoutLevelRenderer`, or what it registered with Fabric's
     * `BuiltinItemRendererRegistry`), as the item renderer draws it with no display transform:
     * JSON {item: [{"tex", "o", "v"}...]}, positions in the model's block units (0 to 1 across
     * the item's cube). The textures drawn with are written into `dir` (`ns/path.png`, their
     * resource path), from the client's resources, for Horizon to draw them as an item model.
     */
    static String drawItems(java.nio.file.Path dir) {
        setUp();
        JsonObject out = new JsonObject();
        Set<String> textures = new HashSet<>();
        int custom = 0;
        for (var item : BuiltInRegistries.ITEM) {
            var id = BuiltInRegistries.ITEM.getKey(item);
            if ("minecraft".equals(id.getNamespace())) continue;
            var stack = new net.minecraft.world.item.ItemStack(item);
            net.minecraft.client.resources.model.BakedModel model;
            try {
                model = items.getItemModelShaper().getItemModel(stack);
            } catch (RuntimeException | LinkageError e) {
                continue;
            }
            if (model == null || !model.isCustomRenderer()) continue;
            custom++;
            Capture capture = new Capture();
            try {
                items.render(stack, net.minecraft.world.item.ItemDisplayContext.NONE, false, new PoseStack(), capture,
                        0xF000F0, net.minecraft.client.renderer.texture.OverlayTexture.NO_OVERLAY, model);
            } catch (RuntimeException | LinkageError e) {
                if (FAILED.add(id.toString())) {
                    Mirror120.log("the item " + id + " didn't draw: " + e);
                    for (StackTraceElement at : e.getStackTrace()) Mirror120.log("    at " + at);
                }
                continue;
            }
            JsonArray meshes = capture.meshes();
            if (meshes.isEmpty()) {
                if (FAILED.add(id.toString())) Mirror120.log("the item " + id + " drew nothing");
                continue;
            }
            for (var mesh : meshes) textures.add(mesh.getAsJsonObject().get("tex").getAsString());
            out.add(id.toString(), meshes);
        }
        Mirror120.log(custom + " mod item(s) drawn by their code, " + out.size() + " drew something");
        if (textures.isEmpty()) return out.toString();
        try (var resources = Loader120.get().clientResources()) {
            for (String texture : textures) {
                var location = net.minecraft.resources.ResourceLocation.tryParse(texture);
                if (location == null) continue;
                var resource = resources.getResource(location);
                if (resource.isEmpty()) continue;
                var to = dir.resolve(location.getNamespace()).resolve(location.getPath());
                try (var in = resource.get().open()) {
                    java.nio.file.Files.createDirectories(to.getParent());
                    java.nio.file.Files.write(to, in.readAllBytes());
                } catch (java.io.IOException e) {
                    Mirror120.log("the texture " + texture + " can't be written: " + e);
                }
            }
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
        // Only those a player could see: vanilla's Entity.shouldRenderAtSqrDistance (64 times
        // the box's mean edge), from the mirror's players here. Horizon draws no farther, and
        // every mob of a level, drawn each tick, was most of the game's time.
        var players = level.players();
        StringBuilder out = new StringBuilder(4096).append('{');
        boolean first = true;
        for (Entity entity : Entities120.of(level).getAll()) {
            if (entity.isRemoved()) continue;
            if (!players.isEmpty() && !seen(entity, players)) continue;
            @SuppressWarnings("unchecked")
            EntityRenderer<Entity> renderer = (EntityRenderer<Entity>) renderers.get(entity.getType());
            if (renderer == null) continue;
            String type = null;
            if (!FAILED.isEmpty()) {
                type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
            }
            Capture capture = new Capture();
            try {
                renderer.render(entity, entity.getYRot(), 1.0f, new PoseStack(), capture, 0xF000F0);
            } catch (RuntimeException | LinkageError e) {
                if (type == null) type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
                if (FAILED.add(type)) {
                    Mirror120.log("a " + type + " didn't draw: " + e);
                    for (StackTraceElement at : e.getStackTrace()) Mirror120.log("    at " + at);
                }
            }
            if (!first) out.append(',');
            first = false;
            out.append('"').append(entity.getId()).append("\":");
            capture.meshes(out);
        }
        return out.append('}').toString();
    }

    /** Whether a player here is near enough to see `entity` (vanilla's render distance). */
    private static boolean seen(Entity entity, List<? extends net.minecraft.world.entity.player.Player> players) {
        double range = entity.getBoundingBox().getSize() * 64.0;
        if (Double.isNaN(range)) range = 64.0;
        double range2 = range * range;
        for (var player : players) {
            if (entity.distanceToSqr(player) < range2) return true;
        }
        return false;
    }

    /** A number as JSON, to four decimals (a vertex's place, UV and normal need no more). */
    static void number(StringBuilder out, double value) {
        long r = Math.round(value * 10000.0);
        if (r < 0) {
            out.append('-');
            r = -r;
        }
        out.append(r / 10000);
        long frac = r % 10000;
        if (frac == 0) return;
        out.append('.');
        int digits = 4;
        while (frac % 10 == 0) {
            frac /= 10;
            digits--;
        }
        String f = Long.toString(frac);
        for (int i = f.length(); i < digits; i++) out.append('0');
        out.append(f);
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
            StringBuilder out = new StringBuilder();
            meshes(out);
            return com.google.gson.JsonParser.parseString(out.toString()).getAsJsonArray();
        }

        void meshes(StringBuilder out) {
            // Quads written on an atlas (a block drawn on a mob) go each to its own sprite's
            // mesh: the stand-in sprites are slots of U (Sprites120), so a quad's U says which.
            Map<String, Mesh> split = new LinkedHashMap<>();
            for (var e : meshes.entrySet()) {
                if (!e.getKey().contains(":textures/atlas/")) {
                    split.merge(e.getKey(), e.getValue(), (a, b) -> a);
                    continue;
                }
                Mesh from = e.getValue();
                for (int q = 0; q + 36 <= from.size; q += 36) {
                    double us = 0;
                    for (int c = 0; c < 4; c++) us += from.v[q + c * 9 + 3];
                    int slot = (int) Math.floor(us / 4);
                    var sprite = Sprites120.slot(slot);
                    if (sprite == null) continue;
                    String texture = sprite.getNamespace() + ":textures/" + sprite.getPath() + ".png";
                    Mesh mesh = split.computeIfAbsent(texture, t -> new Mesh());
                    mesh.overlay = from.overlay;
                    for (int i = 0; i < 36; i++) mesh.add(i % 9 == 3 ? from.v[q + i] - slot : from.v[q + i]);
                }
            }
            out.append('[');
            boolean first = true;
            for (var e : split.entrySet()) {
                Mesh mesh = e.getValue();
                if (mesh.size == 0) continue;
                if (!first) out.append(',');
                first = false;
                out.append("{\"tex\":\"").append(e.getKey()).append("\",\"o\":").append(mesh.overlay).append(",\"v\":[");
                for (int i = 0; i < mesh.size; i++) {
                    if (i > 0) out.append(',');
                    // The ninth of each vertex is its colour, a whole number.
                    if (i % 9 == 8) out.append((long) mesh.v[i]);
                    else number(out, mesh.v[i]);
                }
                out.append("]}");
            }
            out.append(']');
        }
    }

    /** One texture's vertices, as written. */
    private static final class Mesh implements VertexConsumer {
        double[] v = new double[9 * 64];
        int size;

        void add(double value) {
            if (size == v.length) v = java.util.Arrays.copyOf(v, size * 2);
            v[size++] = value;
        }

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
            add(x); add(y); add(z); add(u); add(vv);
            add(nx); add(ny); add(nz);
            add(Integer.toUnsignedLong(fixed ? fixedColor : color));
            color = -1;
        }
    }
}
