package dev.horizonlab.platform;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.horizonlab.bridge.Natives;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;

import java.lang.reflect.Field;

/**
 * Mods' items whose client item is a model type of the mod's own (an `ItemModel` class outside
 * vanilla: Create's `create:model/wrench`, whose quads its Java code picks), as the client's
 * resolver makes them once the models load: each one's quads (vanilla's
 * `ItemModelResolver.updateForTopItem` for a plain stack, every layer's local transform applied,
 * UVs made the sprite's own) and its display transforms. Horizon reads only vanilla's model
 * types, so it drew these blank (in the creative tab, in hand, on the ground); it uses these
 * quads for items its own definition gives nothing.
 */
final class HorizonItemModels {
    private HorizonItemModels() {}

    private static Field layersField, countField, quadsField, localField, transformField, tintsField,
            specialField, argumentField, blockLightField, particleField;

    private static boolean exported;
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("items");

    private static String describe(Object[] layers) throws ReflectiveOperationException {
        StringBuilder b = new StringBuilder(layers.length + " layer(s)");
        for (Object layer : layers) {
            var q = (net.minecraft.client.resources.model.geometry.ItemQuads) quadsField.get(layer);
            Object special = specialField.get(layer);
            b.append(" [").append(q == null ? 0 : q.all().size()).append(" quads")
                    .append(special == null ? "" : ", special " + special.getClass().getName()).append(']');
        }
        return b.toString();
    }

    /**
     * Once the client's models are loaded, as a player's client (HorizonClient's client tick):
     * mods' models read the client player (Create's sand paper, its use).
     */
    static void exportOnce() {
        if (exported) return;
        exported = true;
        export();
    }

    private static void export() {
        try {
            var mc = net.minecraft.client.Minecraft.getInstance();
            var models = mc.getModelManager();
            var resolver = mc.getItemModelResolver();
            if (models == null || resolver == null) return;
            fields();
            JsonObject out = new JsonObject();
            int failed = 0;
            for (var item : BuiltInRegistries.ITEM) {
                var id = BuiltInRegistries.ITEM.getKey(item);
                if (id.getNamespace().equals("minecraft")) continue;
                var stack = new ItemStack(item);
                var modelId = stack.get(DataComponents.ITEM_MODEL);
                if (modelId == null) continue;
                var model = models.getItemModel(modelId);
                if (model == null || model.getClass().getName().startsWith("net.minecraft.")) continue;
                try {
                    var one = item(resolver, stack);
                    if (one != null) out.add(id.toString(), one);
                    else if (DEBUG) Natives.log("Java mods: item " + id + " (" + model.getClass().getSimpleName() + "): no quads; "
                            + describe(layers(resolver, stack, ItemDisplayContext.GUI)));
                } catch (Throwable t) {
                    if (failed++ < 5 || DEBUG) Natives.log("Java mods: item " + id + "'s model couldn't be read: " + t);
                }
            }
            Natives.log("Java mods: " + out.size() + " item(s) drawn with the quads their own model types make"
                    + (out.isEmpty() ? "" : " (" + String.join(", ", out.keySet().stream().limit(8).toList()) + "...)"));
            if (!out.isEmpty()) Natives.modItemModels(out.toString());
        } catch (Throwable t) {
            Natives.log("Java mods: the mods' item models couldn't be read: " + t);
        }
    }

    private static void fields() throws ReflectiveOperationException {
        if (layersField != null) return;
        Class<?> state = net.minecraft.client.renderer.item.ItemStackRenderState.class;
        Class<?> layer = net.minecraft.client.renderer.item.ItemStackRenderState.LayerRenderState.class;
        layersField = open(state, "layers");
        countField = open(state, "activeLayerCount");
        quadsField = open(layer, "quads");
        localField = open(layer, "localTransform");
        transformField = open(layer, "itemTransform");
        tintsField = open(layer, "tintLayers");
        specialField = open(layer, "specialRenderer");
        argumentField = open(layer, "argumentForSpecialRendering");
        blockLightField = open(layer, "usesBlockLight");
        particleField = open(layer, "particleMaterial");
    }

    private static Field open(Class<?> owner, String name) throws NoSuchFieldException {
        Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }

    /** The layers of `stack` resolved for `context`. */
    private static Object[] layers(net.minecraft.client.renderer.item.ItemModelResolver resolver, ItemStack stack,
                                   ItemDisplayContext context) throws ReflectiveOperationException {
        var state = new net.minecraft.client.renderer.item.ItemStackRenderState();
        resolver.updateForTopItem(state, stack, context, null, null, 0);
        Object[] all = (Object[]) layersField.get(state);
        int n = countField.getInt(state);
        Object[] out = new Object[n];
        System.arraycopy(all, 0, out, 0, n);
        return out;
    }

    private static JsonObject item(net.minecraft.client.renderer.item.ItemModelResolver resolver, ItemStack stack)
            throws ReflectiveOperationException {
        Object[] gui = layers(resolver, stack, ItemDisplayContext.GUI);
        JsonArray quads = new JsonArray();
        boolean blockLight = false;
        String particle = null;
        org.joml.Vector3f p = new org.joml.Vector3f();
        for (int l = 0; l < gui.length; l++) {
            Object layer = gui[l];
            var local = (org.joml.Matrix4f) localField.get(layer);
            if (quads.isEmpty()) {
                blockLight = blockLightField.getBoolean(layer);
                var material = (net.minecraft.client.resources.model.sprite.Material.Baked) particleField.get(layer);
                if (material != null) particle = material.sprite().contents().name().toString();
            }
            @SuppressWarnings("unchecked")
            var special = (net.minecraft.client.renderer.special.SpecialModelRenderer<Object>) specialField.get(layer);
            if (special != null) {
                special(special, argumentField.get(layer), local, quads);
                continue;
            }
            var tints = (it.unimi.dsi.fastutil.ints.IntList) tintsField.get(layer);
            mesh(layer, local, tints, quads);
            var list = (net.minecraft.client.resources.model.geometry.ItemQuads) quadsField.get(layer);
            if (list == null || list.isEmpty()) continue;
            bakedQuads(list.all(), local, tints == null ? null : tints.toIntArray(), quads);
        }
        if (quads.isEmpty()) return null;
        JsonObject o = new JsonObject();
        o.add("quads", quads);
        o.addProperty("block_light", blockLight);
        if (particle != null) o.addProperty("particle", particle);
        // Each context's display transform (the first layer's: ModelRenderProperties.applyToLayer).
        JsonObject display = new JsonObject();
        for (var context : ItemDisplayContext.values()) {
            if (context == ItemDisplayContext.NONE) continue;
            Object[] layers = context == ItemDisplayContext.GUI ? gui : layers(resolver, stack, context);
            if (layers.length == 0) continue;
            var t = (net.minecraft.client.resources.model.cuboid.ItemTransform) transformField.get(layers[0]);
            if (t == null || t == net.minecraft.client.resources.model.cuboid.ItemTransform.NO_TRANSFORM) continue;
            JsonArray a = new JsonArray();
            for (var v : new org.joml.Vector3fc[] {t.rotation(), t.translation(), t.scale()}) {
                a.add(round(v.x()));
                a.add(round(v.y()));
                a.add(round(v.z()));
            }
            display.add(context.getSerializedName(), a);
        }
        o.add("display", display);
        return o;
    }

    /** Baked quads under `matrix` (positions), UVs made their sprite's own. */
    private static void bakedQuads(java.util.List<net.minecraft.client.resources.model.geometry.BakedQuad> list, org.joml.Matrix4f local,
                                   int[] tints, JsonArray quads) {
        org.joml.Vector3f p = new org.joml.Vector3f();
        for (var quad : list) {
            var info = quad.materialInfo();
            var sprite = info.sprite();
            if (sprite == null) continue;
            JsonObject q = new JsonObject();
            JsonArray pos = new JsonArray(), uv = new JsonArray();
            float du = sprite.getU1() - sprite.getU0(), dv = sprite.getV1() - sprite.getV0();
            for (int i = 0; i < 4; i++) {
                var at = quad.position(i);
                local.transformPosition(p.set(at.x(), at.y(), at.z()));
                pos.add(round(p.x));
                pos.add(round(p.y));
                pos.add(round(p.z));
                long packed = quad.packedUV(i);
                float u = net.minecraft.client.model.geom.builders.UVPair.unpackU(packed);
                float v = net.minecraft.client.model.geom.builders.UVPair.unpackV(packed);
                uv.add(round(du == 0 ? 0 : (u - sprite.getU0()) / du));
                uv.add(round(dv == 0 ? 0 : (v - sprite.getV0()) / dv));
            }
            q.add("p", pos);
            q.add("uv", uv);
            q.addProperty("s", sprite.contents().name().toString());
            q.addProperty("d", quad.direction().getSerializedName());
            int index = info.tintIndex();
            if (index >= 0 && tints != null && index < tints.length) q.addProperty("c", tints[index] | 0xFF000000);
            if ((info.flags() & net.minecraft.client.resources.model.geometry.BakedQuad.FLAG_TRANSLUCENT) != 0) q.addProperty("t", true);
            if (info.lightEmission() > 0) q.addProperty("e", info.lightEmission());
            if (info.shadeDirectionOverride() != null) q.addProperty("sd", info.shadeDirectionOverride().getSerializedName());
            quads.add(q);
        }
    }

    /**
     * The layer's Fabric mesh (FabricLayerRenderState.emitter: Create's items emit their
     * quads there, not as the layer's quads): each quad in its atlas's UVs, made the quad of
     * the sprite holding its middle.
     */
    private static void mesh(Object layer, org.joml.Matrix4f local, it.unimi.dsi.fastutil.ints.IntList tints, JsonArray quads) {
        Object mesh;
        try {
            var get = layer.getClass().getMethod("fabric_getMutableMesh");
            mesh = get.invoke(layer);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return;
        }
        if (mesh instanceof net.fabricmc.fabric.api.client.renderer.v1.mesh.MeshView view) {
            meshView(view, local, tints == null ? null : tints.toIntArray(), quads);
        }
    }

    /** A Fabric mesh's quads under `local`, each the quad of the sprite holding its middle. */
    private static void meshView(net.fabricmc.fabric.api.client.renderer.v1.mesh.MeshView view, org.joml.Matrix4f local,
                                 int[] tints, JsonArray quads) {
        org.joml.Vector3f p = new org.joml.Vector3f();
        view.forEach(q -> {
            float mu = 0, mv = 0;
            for (int i = 0; i < 4; i++) {
                mu += q.u(i) / 4;
                mv += q.v(i) / 4;
            }
            var atlas = q.atlas() == net.fabricmc.fabric.api.client.renderer.v1.mesh.QuadAtlas.ITEM
                    ? net.minecraft.data.AtlasIds.ITEMS : net.minecraft.data.AtlasIds.BLOCKS;
            var sprite = HorizonEntityRender.spriteAt(atlas, mu, mv);
            if (sprite == null) return;
            float du = sprite.getU1() - sprite.getU0(), dv = sprite.getV1() - sprite.getV0();
            JsonObject o = new JsonObject();
            JsonArray pos = new JsonArray(), uv = new JsonArray();
            for (int i = 0; i < 4; i++) {
                local.transformPosition(p.set(q.x(i), q.y(i), q.z(i)));
                pos.add(round(p.x));
                pos.add(round(p.y));
                pos.add(round(p.z));
                uv.add(round(du == 0 ? 0 : (q.u(i) - sprite.getU0()) / du));
                uv.add(round(dv == 0 ? 0 : (q.v(i) - sprite.getV0()) / dv));
            }
            o.add("p", pos);
            o.add("uv", uv);
            o.addProperty("s", sprite.contents().name().toString());
            var face = q.nominalFace() != null ? q.nominalFace() : q.lightFace();
            o.addProperty("d", face.getSerializedName());
            int color = q.color(0);
            int index = q.tintIndex();
            if (index >= 0 && tints != null && index < tints.length) color = net.minecraft.util.ARGB.multiply(color, tints[index] | 0xFF000000);
            if (color != -1) o.addProperty("c", color);
            if (String.valueOf(q.chunkLayer()).contains("TRANSLUCENT")) o.addProperty("t", true);
            if (q.emissive()) o.addProperty("e", 15);
            if (q.shadeDirectionOverride() != null) o.addProperty("sd", q.shadeDirectionOverride().getSerializedName());
            quads.add(o);
        });
    }

    /**
     * A layer a special renderer draws (Create's items: their quads drawn as custom geometry
     * by the mod's own renderer, as its mixin makes them): its submit into a collector that
     * keeps the custom geometry's quads, each made the quad of the sprite holding its middle
     * (atlas UVs) or of its texture. Models (entity model parts) aren't kept yet.
     */
    private static void special(net.minecraft.client.renderer.special.SpecialModelRenderer<Object> renderer, Object argument,
                                org.joml.Matrix4f local, JsonArray quads) {
        var pose = new com.mojang.blaze3d.vertex.PoseStack();
        pose.last().pose().mul(local);
        pose.last().normal().mul(new org.joml.Matrix3f().set(local).invert().transpose());
        java.lang.reflect.InvocationHandler handler = (proxy, method, args) -> {
            switch (method.getName()) {
                case "order" -> {
                    return proxy;
                }
                // An item's quads the renderer submits itself (vanilla's and Fabric's with the
                // layer's mesh before the foil), posed.
                case "submitItem" -> {
                    var at = new org.joml.Matrix4f(((com.mojang.blaze3d.vertex.PoseStack) args[0]).last().pose());
                    var itemQuads = (net.minecraft.client.resources.model.geometry.ItemQuads) args[6];
                    if (itemQuads != null) bakedQuads(itemQuads.all(), at, (int[]) args[5], quads);
                    if (args.length == 9 && args[7] instanceof net.fabricmc.fabric.api.client.renderer.v1.mesh.MeshView view) {
                        meshView(view, at, (int[]) args[5], quads);
                    }
                    return null;
                }
                case "submitCustomGeometry" -> {
                    geometry((com.mojang.blaze3d.vertex.PoseStack) args[0],
                            (net.minecraft.client.renderer.rendertype.RenderType) args[1],
                            (net.minecraft.client.renderer.SubmitNodeCollector.CustomGeometryRenderer) args[2], quads);
                    return null;
                }
                case "hashCode" -> {
                    return System.identityHashCode(proxy);
                }
                case "equals" -> {
                    return proxy == args[0];
                }
                case "toString" -> {
                    return "HorizonItemModels.collector";
                }
                default -> {
                    if (method.isDefault() && !method.getName().equals("submitItem")) {
                        return java.lang.reflect.InvocationHandler.invokeDefault(proxy, method, args);
                    }
                    return null;
                }
            }
        };
        var collector = (net.minecraft.client.renderer.SubmitNodeCollector) java.lang.reflect.Proxy.newProxyInstance(
                HorizonItemModels.class.getClassLoader(), new Class<?>[] {net.minecraft.client.renderer.SubmitNodeCollector.class}, handler);
        renderer.submit(argument, pose, collector, net.minecraft.util.LightCoordsUtil.FULL_BRIGHT,
                net.minecraft.client.renderer.texture.OverlayTexture.NO_OVERLAY, false, 0);
    }

    /** One custom geometry's quads, into Horizon's item quads (see special). */
    private static void geometry(com.mojang.blaze3d.vertex.PoseStack pose, net.minecraft.client.renderer.rendertype.RenderType type,
                                 net.minecraft.client.renderer.SubmitNodeCollector.CustomGeometryRenderer renderer, JsonArray quads)
            throws ReflectiveOperationException {
        if (!String.valueOf(type.primitiveTopology()).contains("QUAD")) return;
        String texture = HorizonEntityRender.texture(type);
        if (texture == null) return;
        var recorder = new HorizonEntityRender.Recorder();
        renderer.render(pose.last(), recorder);
        int n = recorder.vertices.size() / 4 * 4;
        int colon = texture.indexOf(':');
        String ns = texture.substring(0, colon), path = texture.substring(colon + 1).replace(".png", "");
        boolean atlas = path.startsWith("textures/atlas/");
        var atlasId = net.minecraft.resources.Identifier.fromNamespaceAndPath(ns, path.replace("textures/atlas/", ""));
        boolean translucent = String.valueOf(type).contains("translucent");
        for (int q = 0; q < n; q += 4) {
            float mu = 0, mv = 0;
            for (int i = 0; i < 4; i++) {
                mu += recorder.vertices.get(q + i)[3] / 4;
                mv += recorder.vertices.get(q + i)[4] / 4;
            }
            String name;
            float u0 = 0, v0 = 0, du = 1, dv = 1;
            if (atlas) {
                var sprite = HorizonEntityRender.spriteAt(atlasId, mu, mv);
                if (sprite == null) continue;
                name = sprite.contents().name().toString();
                u0 = sprite.getU0();
                v0 = sprite.getV0();
                du = sprite.getU1() - u0;
                dv = sprite.getV1() - v0;
            } else {
                if (!path.startsWith("textures/")) continue;
                name = ns + ":" + path.substring("textures/".length());
            }
            JsonObject o = new JsonObject();
            JsonArray pos = new JsonArray(), uv = new JsonArray();
            float nx = 0, ny = 0, nz = 0;
            int color = -1;
            for (int i = 0; i < 4; i++) {
                float[] x = recorder.vertices.get(q + i);
                pos.add(round(x[0]));
                pos.add(round(x[1]));
                pos.add(round(x[2]));
                uv.add(round(du == 0 ? 0 : (x[3] - u0) / du));
                uv.add(round(dv == 0 ? 0 : (x[4] - v0) / dv));
                nx += x[5];
                ny += x[6];
                nz += x[7];
                color = Float.floatToRawIntBits(x[8]);
            }
            o.add("p", pos);
            o.add("uv", uv);
            o.addProperty("s", name);
            o.addProperty("d", net.minecraft.core.Direction.getApproximateNearest(nx, ny, nz).getSerializedName());
            if (color != -1) o.addProperty("c", color);
            if (translucent) o.addProperty("t", true);
            quads.add(o);
        }
    }

    private static float round(float f) {
        return Math.round(f * 100000.0F) / 100000.0F;
    }
}
