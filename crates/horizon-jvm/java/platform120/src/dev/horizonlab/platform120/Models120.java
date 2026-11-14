package dev.horizonlab.platform120;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.block.BlockModelShaper;
import net.minecraft.client.renderer.block.model.BlockModel;
import net.minecraft.client.resources.model.BakedModel;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.BlockAndTintGetter;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;

/**
 * The 1.20.1 mods' blocks whose look is their code (phase 7): a model a mod makes per position
 * (LibMultipart's container, whose parts are in its block entity: Wired Redstone's cables), which
 * Horizon can't draw from data.
 *
 * <p>The client's block models are baked as the client bakes them (ModelManager's loaders over
 * vanilla's and the mods' resource packs, Fabric's model loading hooked into ModelBakery as in a
 * real client), with stand-in sprites (Sprites120: each its own slot of UVs). A mod's state whose
 * model is a Fabric model that isn't a plain vanilla one is drawn by code. Such a block is
 * rendered at its position with Indigo's own block renderer, its world the mirror level
 * (Horizon's blocks and block entities), into a consumer that keeps the quads: positions in the
 * block, the sprite and the UVs in it (from the slot), the colour (the mod's tint included).
 */
final class Models120 {
    private Models120() {}

    private static Map<BlockState, BakedModel> models;
    private static List<BlockState> drawnByCode;

    /** Every state's baked client model (vanilla's and the mods'), baking them first. */
    private static Map<ResourceLocation, BakedModel> topLevel;

    /** Every model baked, by location: the items' too (`ns:item#inventory`). */
    static synchronized Map<ResourceLocation, BakedModel> topLevel() {
        bake();
        return topLevel;
    }

    static synchronized Map<BlockState, BakedModel> baked() {
        bake();
        return models;
    }

    /** The mods' states drawn by code, as Horizon names states. Bakes the models first. */
    static synchronized String[] drawnByCode() {
        bake();
        String[] out = new String[drawnByCode.size()];
        for (int i = 0; i < out.length; i++) out[i] = net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(drawnByCode.get(i));
        return out;
    }

    private static void bake() {
        if (models != null) return;
        var started = System.nanoTime();
        var client = net.minecraft.client.Minecraft.getInstance();
        if (client.getBlockColors() == null) {
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setBlockColors(
                    net.minecraft.client.color.block.BlockColors.createDefault());
        }
        MultiPackResourceManager resources = clientResources();
        var executor = net.minecraft.Util.backgroundExecutor();
        Map<ResourceLocation, BlockModel> blockModels =
                dev.horizonlab.platform120.mixin.ModelManagerAccess.horizon$loadBlockModels(resources, executor).join();
        Map<ResourceLocation, List<ModelBakery.LoadedJson>> blockStates =
                dev.horizonlab.platform120.mixin.ModelManagerAccess.horizon$loadBlockStates(resources, executor).join();
        // Fabric's model loading: its plugins (LibMultipart's container model among them) are
        // prepared as a real reload prepares them, and its ModelBakery hooks read them here.
        var plugins = net.fabricmc.fabric.impl.client.model.loading.ModelLoadingPluginManager.CURRENT_PLUGINS;
        plugins.set(net.fabricmc.fabric.impl.client.model.loading.ModelLoadingPluginManager.preparePlugins(resources, executor).join());
        ModelBakery bakery;
        try {
            bakery = new ModelBakery(client.getBlockColors(), NoProfiler.INSTANCE, blockModels, blockStates);
            bakery.bakeModels((location, material) -> Sprites120.sprite(material));
        } finally {
            plugins.remove();
        }
        Map<ResourceLocation, BakedModel> baked = bakery.getBakedTopLevelModels();
        topLevel = baked;
        // The client's model manager over them: a mod's model fetches its extra parts from it
        // as it draws (Wired Redstone's gates, through Fabric's BakedModelManagerHelper).
        if (client.getModelManager() == null) {
            var manager = Allocate.make(net.minecraft.client.resources.model.ModelManager.class);
            var access = (dev.horizonlab.platform120.mixin.ModelManagerAccess) manager;
            access.horizon$setBakedRegistry(baked);
            access.horizon$setMissingModel(baked.get(ModelBakery.MISSING_MODEL_LOCATION));
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$setModelManager(manager);
        }
        models = new IdentityHashMap<>();
        drawnByCode = new ArrayList<>();
        for (var block : BuiltInRegistries.BLOCK) {
            boolean mod = !"minecraft".equals(BuiltInRegistries.BLOCK.getKey(block).getNamespace());
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                BakedModel model = baked.get(BlockModelShaper.stateToModelLocation(state));
                if (model == null) continue;
                models.put(state, model);
                if (mod && model instanceof net.fabricmc.fabric.api.renderer.v1.model.FabricBakedModel fabric
                        && !fabric.isVanillaAdapter()) {
                    drawnByCode.add(state);
                }
            }
        }
        Mirror120.log("the client's block models baked in " + (System.nanoTime() - started) / 1_000_000 + " ms: "
                + models.size() + " states, " + drawnByCode.size() + " of the mods' drawn by code");
    }

    /** Vanilla's and the mods' client resource packs, as a client loads them. */
    static MultiPackResourceManager clientResources() {
        List<RepositorySource> sources = new ArrayList<>();
        sources.add(new net.minecraft.client.resources.ClientPackSource(java.nio.file.Path.of("assets-index-unused")));
        try {
            Class<?> creator = Class.forName("net.fabricmc.fabric.impl.resource.loader.ModResourcePackCreator");
            sources.add((RepositorySource) creator.getConstructor(PackType.class).newInstance(PackType.CLIENT_RESOURCES));
        } catch (ReflectiveOperationException | RuntimeException e) {
            Mirror120.log("the mods' resource packs can't be listed: " + e);
        }
        PackRepository repository = new PackRepository(sources.toArray(new RepositorySource[0]));
        repository.reload();
        List<String> selected = new ArrayList<>();
        for (var pack : repository.getAvailablePacks()) {
            if (pack.isRequired() || pack.getId().equals("vanilla") || pack.getId().startsWith("fabric")
                    || pack.getDefaultPosition() == net.minecraft.server.packs.repository.Pack.Position.TOP) {
                selected.add(pack.getId());
            }
        }
        // Every mod's own pack (Fabric names them by mod id), beside vanilla's.
        for (var pack : repository.getAvailablePacks()) {
            if (!selected.contains(pack.getId()) && !pack.getId().equals("programmer_art") && !pack.getId().equals("high_contrast")) {
                selected.add(pack.getId());
            }
        }
        repository.setSelected(selected);
        return new MultiPackResourceManager(PackType.CLIENT_RESOURCES, repository.openAllSelected());
    }

    /**
     * The quads of the block at `pos` of a mirror level, as its model draws it there: for each
     * quad its sprite, and per corner x, y, z (in the block), u, v (in the sprite, 0..1) and the
     * colour (ARGB). Empty when its model isn't one drawn by code.
     */
    static Object[] render(Level level, BlockPos pos) {
        bake();
        BlockState state = level.getBlockState(pos);
        BakedModel model = models.get(state);
        Capture capture = new Capture();
        if (model != null) {
            freshModelKeys(level.getBlockEntity(pos));
            var context = new net.fabricmc.fabric.impl.client.indigo.renderer.render.BlockRenderContext();
            var options = net.minecraft.client.Minecraft.getInstance().options;
            // Horizon lights and shades the quads itself: they come without ambient occlusion.
            Boolean ao = options == null ? null : options.ambientOcclusion().get();
            if (options != null) options.ambientOcclusion().set(false);
            try {
                context.render(new View(level), model, state, pos, new PoseStack(), capture, false,
                        RandomSource.create(), state.getSeed(pos), net.minecraft.client.renderer.texture.OverlayTexture.NO_OVERLAY);
            } catch (RuntimeException | LinkageError e) {
                // A crash report wraps the mod's own failure: say that, and where it was.
                Throwable cause = e;
                while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
                var at = cause.getStackTrace().length > 0 ? " at " + cause.getStackTrace()[0] : "";
                Mirror120.log("the block at " + pos.toShortString() + " (" + state + ") didn't draw: " + cause + at);
            } finally {
                if (options != null && ao != null) options.ambientOcclusion().set(ao);
            }
        }
        return capture.answer();
    }

    /**
     * LibMultipart's container keeps its parts' model keys (what its model draws) and makes them
     * again only in a client world, when the server tells it to redraw. The mirror is the server
     * side, so the keys stayed what they were when the block loaded: a Wired Redstone cable drew
     * no arm after it linked to its neighbour. They are made again from the parts, as the client's
     * redraw makes them, before the block is drawn. (LibMultipart's own names, not Minecraft's.)
     */
    private static void freshModelKeys(BlockEntity entity) {
        if (entity == null || !"alexiil.mc.lib.multipart.impl.MultipartBlockEntity".equals(entity.getClass().getName())) return;
        try {
            var containerField = entity.getClass().getDeclaredField("container");
            containerField.setAccessible(true);
            Object container = containerField.get(entity);
            var parts = (List<?>) container.getClass().getField("parts").get(container);
            var keys = com.google.common.collect.ImmutableList.builder();
            for (Object holder : parts) {
                Object part = holder.getClass().getField("part").get(holder);
                Object key = part.getClass().getMethod("getModelKey").invoke(part);
                if (key != null) keys.add(key);
            }
            var keysField = container.getClass().getDeclaredField("partModelKeys");
            keysField.setAccessible(true);
            keysField.set(container, keys.build());
        } catch (ReflectiveOperationException | RuntimeException e) {
            Mirror120.log("LibMultipart's model keys can't be made again: " + e);
        }
    }

    /** The world a block draws in: the mirror level's blocks, full light, no shading of its own. */
    private record View(Level level) implements BlockAndTintGetter {
        @Override public float getShade(Direction side, boolean shade) { return 1.0f; }
        @Override public LevelLightEngine getLightEngine() { return null; }
        @Override public int getBrightness(LightLayer layer, BlockPos pos) { return 15; }
        @Override public int getRawBrightness(BlockPos pos, int dark) { return 15; }
        @Override public boolean canSeeSky(BlockPos pos) { return true; }
        @Override public int getBlockTint(BlockPos pos, ColorResolver resolver) {
            return resolver.getColor(level.getBiome(pos).value(), pos.getX(), pos.getZ());
        }
        @Override public BlockEntity getBlockEntity(BlockPos pos) { return level.getBlockEntity(pos); }
        @Override public BlockState getBlockState(BlockPos pos) { return level.getBlockState(pos); }
        @Override public FluidState getFluidState(BlockPos pos) { return level.getFluidState(pos); }
        @Override public int getHeight() { return level.getHeight(); }
        @Override public int getMinBuildHeight() { return level.getMinBuildHeight(); }
    }

    /** Keeps what is drawn: four corners a quad. */
    private static final class Capture implements VertexConsumer {
        final List<Double> corners = new ArrayList<>();
        final List<Integer> colors = new ArrayList<>();
        double x, y, z;
        float u, v;
        int color = -1;
        boolean fixed;
        int fixedColor = -1;

        @Override public VertexConsumer vertex(double x, double y, double z) { this.x = x; this.y = y; this.z = z; return this; }
        @Override public VertexConsumer color(int r, int g, int b, int a) { color = a << 24 | r << 16 | g << 8 | b; return this; }
        @Override public VertexConsumer uv(float u, float v) { this.u = u; this.v = v; return this; }
        @Override public VertexConsumer overlayCoords(int u, int v) { return this; }
        @Override public VertexConsumer uv2(int u, int v) { return this; }
        @Override public VertexConsumer normal(float x, float y, float z) { return this; }
        @Override public void defaultColor(int r, int g, int b, int a) { fixed = true; fixedColor = a << 24 | r << 16 | g << 8 | b; }
        @Override public void unsetDefaultColor() { fixed = false; }

        @Override public void endVertex() {
            corners.add(x);
            corners.add(y);
            corners.add(z);
            corners.add((double) u);
            corners.add((double) v);
            colors.add(fixed ? fixedColor : color);
            color = -1;
        }

        /** {sprites (one a quad), corners (x y z u v, four a quad), colours (one a corner)}. */
        Object[] answer() {
            int quads = colors.size() / 4;
            String[] sprites = new String[quads];
            double[] flat = new double[quads * 20];
            int[] argb = new int[quads * 4];
            for (int q = 0; q < quads; q++) {
                double us = 0;
                for (int c = 0; c < 4; c++) us += corners.get((q * 4 + c) * 5 + 3);
                int slot = (int) Math.floor(us / 4);
                ResourceLocation sprite = Sprites120.slot(slot);
                sprites[q] = sprite == null ? "minecraft:missingno" : sprite.toString();
                for (int c = 0; c < 4; c++) {
                    int at = (q * 4 + c) * 5;
                    flat[q * 20 + c * 5] = corners.get(at);
                    flat[q * 20 + c * 5 + 1] = corners.get(at + 1);
                    flat[q * 20 + c * 5 + 2] = corners.get(at + 2);
                    flat[q * 20 + c * 5 + 3] = corners.get(at + 3) - slot;
                    flat[q * 20 + c * 5 + 4] = corners.get(at + 4);
                    argb[q * 4 + c] = colors.get(q * 4 + c);
                }
            }
            return new Object[] {sprites, flat, argb};
        }
    }
}
