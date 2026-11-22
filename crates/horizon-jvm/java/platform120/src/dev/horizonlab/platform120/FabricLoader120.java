package dev.horizonlab.platform120;

import java.util.List;
import java.util.Map;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.event.player.UseEntityCallback;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.model.BakedModel;
import net.minecraft.core.BlockPos;
import net.minecraft.network.protocol.Packet;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.EntityHitResult;

/**
 * {@link Loader120} under Fabric: Fabric API's events and registries, through the platform's
 * Fabric modules (Events120, Keys120's registry, Hoppers120, Packets120, Models120).
 */
final class FabricLoader120 implements Loader120 {
    @Override public void serverStart(MinecraftServer server, Iterable<ServerLevel> levels) { Events120.start(server, levels); }
    @Override public boolean serverStarted() { return Events120.started(); }
    @Override public void levelLoaded(MinecraftServer server, ServerLevel level) { Events120.loaded(server, level); }
    @Override public void playerJoined(MinecraftServer server, ServerPlayer player) { Events120.joined(server, player); }
    @Override public void playerLeft(MinecraftServer server, ServerPlayer player) { Events120.left(server, player); }
    @Override public void around(ServerLevel level, BlockPos pos) { Events120.around(level, pos); }
    @Override public boolean anyTickListeners() { return Events120.anyTickListeners(); }
    @Override public void serverTick(MinecraftServer server, ServerLevel level, long tick) { Events120.tick(server, level, tick); }

    @Override
    public boolean clientTickListeners() {
        return Events120.listeners(ClientTickEvents.START_CLIENT_TICK) != 0
                || Events120.listeners(ClientTickEvents.END_CLIENT_TICK) != 0;
    }

    @Override
    public void clientTick(Minecraft minecraft) {
        Events120.fire(ClientTickEvents.START_CLIENT_TICK, "client tick start", l -> l.onStartTick(minecraft));
        Events120.fire(ClientTickEvents.END_CLIENT_TICK, "client tick end", l -> l.onEndTick(minecraft));
    }

    @Override
    @SuppressWarnings("unchecked")
    public List<KeyMapping> modKeyMappings() {
        try {
            var field = net.fabricmc.fabric.impl.client.keybinding.KeyBindingRegistryImpl.class
                    .getDeclaredField("MODDED_KEY_BINDINGS");
            field.setAccessible(true);
            return (List<KeyMapping>) field.get(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            Mirror120.log("the mods' key bindings could not be read: " + e);
            return List.of();
        }
    }

    @Override
    public InteractionResult useEntity(ServerPlayer player, ServerLevel level, InteractionHand hand, Entity target, EntityHitResult hit) {
        return UseEntityCallback.EVENT.invoker().interact(player, level, hand, target, hit);
    }

    @Override
    public boolean beforeBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        return PlayerBlockBreakEvents.BEFORE.invoker().beforeBlockBreak(level, player, pos, state, entity);
    }

    @Override
    public void canceledBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        PlayerBlockBreakEvents.CANCELED.invoker().onBlockBreakCanceled(level, player, pos, state, entity);
    }

    @Override
    public void afterBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        PlayerBlockBreakEvents.AFTER.invoker().afterBlockBreak(level, player, pos, state, entity);
    }

    @Override public int hopperInsert(ServerLevel level, BlockPos pos, int face, List<ItemStack> stacks) { return Hoppers120.insert(level, pos, face, stacks); }
    @Override public ItemStack hopperExtract(ServerLevel level, BlockPos pos, int face, boolean anyRoom, List<ItemStack> partial) { return Hoppers120.extract(level, pos, face, anyRoom, partial); }

    @Override public void toClient(ServerPlayer player, Packet<?> packet) { Packets120.toClient(player, packet); }
    @Override public void fromClient(Packet<?> packet) { Packets120.fromClient(packet); }

    @Override public MultiPackResourceManager clientResources() { return Models120.clientResources(); }
    @Override public Map<ResourceLocation, BakedModel> topLevel() { return Models120.topLevel(); }
    @Override public Map<BlockState, BakedModel> baked() { return Models120.baked(); }
    @Override public String[] drawnByCode() { return Models120.drawnByCode(); }
    @Override public Object[] render(Level level, BlockPos pos) { return Models120.render(level, pos); }

    @Override
    public net.minecraft.server.packs.repository.RepositorySource modPacks(net.minecraft.server.packs.PackType type) {
        try {
            Class<?> creator = Class.forName("net.fabricmc.fabric.impl.resource.loader.ModResourcePackCreator");
            return (net.minecraft.server.packs.repository.RepositorySource) creator.getConstructor(
                    net.minecraft.server.packs.PackType.class).newInstance(type);
        } catch (ReflectiveOperationException | RuntimeException e) {
            Mirror120.log("the mods' " + type + " packs can't be listed: " + e);
            return null;
        }
    }

    @Override
    public void finalizeBiomes(net.minecraft.core.RegistryAccess registries) {
        net.fabricmc.fabric.impl.biome.modification.BiomeModificationImpl.INSTANCE.finalizeWorldGen(registries);
    }

    @Override public int firstEntityId() { return Entities120.FIRST_ID; }

    /**
     * Vanilla's, and those mods add through Fabric API's DynamicRegistries (which keeps its own
     * list; its mixins hand it to a real server).
     */
    @Override
    @SuppressWarnings("unchecked")
    public java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>> worldgenRegistries() {
        try {
            Class<?> fabric = Class.forName("net.fabricmc.fabric.impl.registry.sync.DynamicRegistriesImpl");
            return (java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>>) fabric.getMethod("getDynamicRegistries").invoke(null);
        } catch (ReflectiveOperationException e) {
            return net.minecraft.resources.RegistryDataLoader.WORLDGEN_REGISTRIES;
        }
    }

    // Probes of the loader's networking and key bindings (the bridge's own checks).

    private static volatile int probeClientGot = -1;
    private static volatile int probeServerGot = -1;
    private static boolean probeRegistered;
    private static boolean keyProbeRegistered;
    private static volatile int keyProbeClicks;

    @Override
    public Object[] packetProbe(ServerPlayer player) {
            ResourceLocation down = new ResourceLocation("horizon", "probe_down");
            ResourceLocation up = new ResourceLocation("horizon", "probe_up");
            if (!probeRegistered) {
                probeRegistered = true;
                net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.registerGlobalReceiver(down,
                        (client, handler, buf, sender) -> {
                            int got = buf.readVarInt();
                            client.execute(() -> {
                                probeClientGot = got;
                                var reply = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
                                reply.writeVarInt(got + 1);
                                net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.send(up, reply);
                            });
                        });
                net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.registerGlobalReceiver(up,
                        (server, from, handler, buf, sender) -> {
                            int got = buf.readVarInt();
                            server.execute(() -> probeServerGot = got);
                        });
            }
            probeClientGot = -1;
            probeServerGot = -1;
            boolean canDown = net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.canSend(player, down);
            var buf = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
            buf.writeVarInt(41);
            net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.send(player, down, buf);
            boolean canUp = net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.canSend(up);
        return new Object[] {probeClientGot, probeServerGot, canDown, canUp};
    }

    @Override
    public Object[] keyProbe() {
        if (!keyProbeRegistered) {
            keyProbeRegistered = true;
            // Mods register their keys while they load, before the options exist; Fabric refuses
            // one after. The probe comes later, so it registers with the options set aside.
            var client = net.minecraft.client.Minecraft.getInstance();
            var options = client.options;
            var access = (dev.horizonlab.platform120.mixin.MinecraftAccess) client;
            access.horizon$setOptions(null);
            net.minecraft.client.KeyMapping mapping;
            try {
                mapping = net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper.registerKeyBinding(
                        new net.minecraft.client.KeyMapping("key.horizon.probe", 75, "key.categories.misc"));
            } finally {
                access.horizon$setOptions(options);
            }
            var probe = mapping;
            net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK.register(ticking -> {
                while (probe.consumeClick()) keyProbeClicks++;
            });
        }
        return new Object[] {keyProbeClicks};
    }

    /** Fabric's client item API's ItemTooltipCallback, by reflection: the platform jar is built without it. */
    @Override
    public void tooltip(ItemStack stack, net.minecraft.world.item.TooltipFlag flag, List<net.minecraft.network.chat.Component> all) {
        try {
            Class<?> callback = Class.forName("net.fabricmc.fabric.api.client.item.v1.ItemTooltipCallback");
            Object eventObject = callback.getField("EVENT").get(null);
            Object invoker = eventObject.getClass().getMethod("invoker").invoke(eventObject);
            var getTooltip = callback.getMethod("getTooltip", ItemStack.class,
                    net.minecraft.world.item.TooltipFlag.class, java.util.List.class);
            getTooltip.invoke(invoker, stack, flag, all);
        } catch (ClassNotFoundException e) {
            // No Fabric client item API: no tooltip mods.
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e.getCause() != null ? e.getCause() : e);
        }
    }

    /** Fabric API's ServerLivingEntityEvents.AFTER_DEATH, by reflection. */
    @Override
    public int afterDeath(net.minecraft.world.entity.LivingEntity mob, net.minecraft.world.damagesource.DamageSource source) {
        try {
            Object event = Class.forName("net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents")
                    .getField("AFTER_DEATH").get(null);
            Object invoker = event.getClass().getMethod("invoker").invoke(event);
            for (var m : invoker.getClass().getMethods()) {
                if (m.getName().equals("afterDeath") && m.getParameterCount() == 2) {
                    m.setAccessible(true);
                    m.invoke(invoker, mob, source);
                    return 1;
                }
            }
            return 0;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e.getCause() != null ? e.getCause() : e);
        }
    }

    @Override
    public boolean listensToDamage() {
        return allowDamageInvoker() != null;
    }

    /** Fabric API's ServerLivingEntityEvents.ALLOW_DAMAGE. */
    @Override
    public boolean allowDamage(net.minecraft.world.entity.LivingEntity mob, net.minecraft.world.damagesource.DamageSource source, float amount) {
        Object allow = allowDamageInvoker();
        if (allow == null) return true;
        try {
            for (var m : allow.getClass().getMethods()) {
                if (m.getName().equals("allowDamage") && m.getParameterCount() == 3) {
                    m.setAccessible(true);
                    return !Boolean.FALSE.equals(m.invoke(allow, mob, source, amount));
                }
            }
            return true;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e.getCause() != null ? e.getCause() : e);
        }
    }

    /**
     * Fabric's ALLOW_DAMAGE invoker, or null without Fabric's entity events or a mod listening
     * (its ArrayBackedEvent's `handlers` empty).
     */
    private static Object allowDamageInvoker() {
        try {
            Object event = Class.forName("net.fabricmc.fabric.api.entity.event.v1.ServerLivingEntityEvents")
                    .getField("ALLOW_DAMAGE").get(null);
            for (Class<?> c = event.getClass(); c != null; c = c.getSuperclass()) {
                try {
                    var field = c.getDeclaredField("handlers");
                    field.setAccessible(true);
                    if (field.get(event) instanceof Object[] handlers && handlers.length == 0) return null;
                    break;
                } catch (NoSuchFieldException e) {
                    // Up the classes.
                }
            }
            return event.getClass().getMethod("invoker").invoke(event);
        } catch (ReflectiveOperationException | RuntimeException | LinkageError e) {
            return null;
        }
    }

    @Override
    public Map<String, java.util.function.Function<net.minecraft.client.gui.screens.Screen, net.minecraft.client.gui.screens.Screen>> settingsScreens() {
        return Config120.modMenuFactories();
    }

    @Override
    public String modName(String id) {
        return Config120.modMenuName(id);
    }
}
