package dev.horizonlab.platform120;

import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.transformer.meta.MixinMerged;

/**
 * What the 1.20.1 mods draw on a player's HUD (phase 5i): AppleSkin's food and saturation over
 * the food bar, Farmer's Delight's nourishment and comfort, a mod's {@code HudRenderCallback}.
 *
 * <p>Horizon draws the HUD itself; the mods draw theirs into vanilla's in 1.20.1 (mixins into
 * `Gui`'s parts) or after it (Fabric's callback, fired from its own handler at the end of
 * `Gui.render`). So the parts mods inject into run here, with the player's numbers as Horizon
 * has them, and only what the mods draw is kept ({@link Graphics120#modsOnly}): vanilla's own
 * hearts and bars are dropped as they're drawn. Then every handler merged into `Gui.render`
 * runs, Fabric's (the callback) and the mods' (Balm's). As the 26.3 bridge's
 * `HorizonClient.hud`.
 *
 * <p>The client's in-game GUI is made without its constructor, as the rest of this client is:
 * the fields its parts read are set here. While a frame is drawn the client has a level (an
 * empty one: the hearts ask whether the world is hardcore) and its camera is the player.
 */
public final class Hud120 {
    private Hud120() {}

    private static Gui gui;
    private static net.minecraft.client.multiplayer.ClientLevel level;
    /** The parts and handlers that failed, each said three times at most. */
    private static final java.util.Map<String, Integer> FAILURES = new java.util.HashMap<>();
    /** HORIZON_120_HUD: the kept and dropped operations of each frame, in the runtime's log. */
    private static final boolean DEBUG = System.getenv("HORIZON_120_HUD") != null;

    /**
     * The mods' HUD for `player` at a `width`x`height` GUI: the frame's operations as JSON.
     * `state` is the player as Horizon has them (health, food, air, experience).
     */
    public static synchronized String frame(ServerPlayer player, String state, int width, int height) {
        JsonObject s = com.google.gson.JsonParser.parseString(state).getAsJsonObject();
        stats(player, s);
        boolean hurtable = s.has("hurtable") && s.get("hurtable").getAsBoolean();
        Minecraft minecraft = Minecraft.getInstance();
        Screens120.font();
        Screens120.clientPlayer(player);
        Screens120.pointer(width, height, -1, -1);
        var local = minecraft.player;
        share(player, local);
        Gui gui = gui(minecraft);
        var access = (dev.horizonlab.platform120.mixin.GuiAccess) gui;
        access.horizon$setScreenWidth(width);
        access.horizon$setScreenHeight(height);
        access.horizon$setTickCount(player.tickCount);
        var levelBefore = minecraft.level;
        var cameraBefore = minecraft.cameraEntity;
        minecraft.level = clientLevel(player.serverLevel());
        minecraft.cameraEntity = local;
        // The client's tick before its frame, as Minecraft.runTick comes before the render:
        // what the player looks at (`hitResult`) and Fabric's client tick events, where
        // overlays about the world find it (Jade's).
        if (TIMER != null) TIMER.tickDelta = 1f;
        part("client tick", () -> clientTick(minecraft, player, local, s));
        Graphics120 recording = Graphics120.begin();
        recording.modsOnly = true;
        try {
            GuiGraphics graphics = Screens120.recorder();
            // In Gui.render's order: the hotbar, the hearts and food where the player can be
            // hurt, the experience bar, the effects, then the handlers at its end.
            part("renderHotbar", () -> access.horizon$renderHotbar(0f, graphics));
            if (hurtable) {
                part("renderPlayerHealth", () -> access.horizon$renderPlayerHealth(graphics));
            }
            part("renderExperienceBar", () -> access.horizon$renderExperienceBar(graphics, width / 2 - 91));
            part("renderEffects", () -> access.horizon$renderEffects(graphics));
            renderHandlers(gui, graphics);
            if (DEBUG) {
                System.out.println("Hud120: " + recording.dropped + " vanilla operations left out, kept "
                        + recording.finishedCount());
            }
            String frame = recording.finish().toString();
            if (DEBUG) System.out.println("Hud120: frame " + frame.substring(0, Math.min(frame.length(), 3000)));
            return frame;
        } finally {
            if (Graphics120.current() == recording) recording.finish();
            minecraft.level = levelBefore;
            minecraft.cameraEntity = cameraBefore;
        }
    }

    /** Horizon's numbers on the runtime's player, which the client player reads. */
    static void stats(ServerPlayer player, JsonObject s) {
        if (s.has("pos") && s.get("pos").isJsonArray() && s.getAsJsonArray("pos").size() == 3) {
            var pos = s.getAsJsonArray("pos");
            float yaw = s.has("yaw") ? s.get("yaw").getAsFloat() : player.getYRot();
            float pitch = s.has("pitch") ? s.get("pitch").getAsFloat() : player.getXRot();
            player.moveTo(pos.get(0).getAsDouble(), pos.get(1).getAsDouble(), pos.get(2).getAsDouble(), yaw, pitch);
            player.setYHeadRot(yaw);
        }
        var maxHealth = player.getAttribute(net.minecraft.world.entity.ai.attributes.Attributes.MAX_HEALTH);
        if (maxHealth != null && s.has("max_health")) maxHealth.setBaseValue(s.get("max_health").getAsDouble());
        if (s.has("health")) player.setHealth(s.get("health").getAsFloat());
        if (s.has("absorption")) player.setAbsorptionAmount(s.get("absorption").getAsFloat());
        if (s.has("air")) player.setAirSupply(s.get("air").getAsInt());
        var food = player.getFoodData();
        if (s.has("food")) food.setFoodLevel(s.get("food").getAsInt());
        if (s.has("saturation")) food.setSaturation(s.get("saturation").getAsFloat());
        if (s.has("exhaustion")) {
            ((dev.horizonlab.platform120.mixin.FoodDataAccess) food).horizon$setExhaustion(s.get("exhaustion").getAsFloat());
        }
        if (s.has("xp_level")) player.experienceLevel = s.get("xp_level").getAsInt();
        if (s.has("xp_progress")) player.experienceProgress = s.get("xp_progress").getAsFloat();
    }

    /**
     * The client player as the server player: what the HUD's parts and mods read of it (health
     * and air in its synced data, food, attributes, effects, abilities, what its eyes are in).
     */
    private static void share(ServerPlayer player, net.minecraft.client.player.LocalPlayer local) {
        var entity = (dev.horizonlab.platform120.mixin.EntityAccess) (Object) local;
        entity.horizon$setEntityData(player.getEntityData());
        entity.horizon$setLevel(clientLevel(player.serverLevel()));
        entity.horizon$setFluidOnEyes(((dev.horizonlab.platform120.mixin.EntityAccess) (Object) player).horizon$fluidOnEyes());
        var living = (dev.horizonlab.platform120.mixin.LivingEntityAccess) (Object) local;
        living.horizon$setAttributes(player.getAttributes());
        living.horizon$setActiveEffects(effects(player));
        var asPlayer = (dev.horizonlab.platform120.mixin.PlayerAccess) (Object) local;
        asPlayer.horizon$setFoodData(player.getFoodData());
        asPlayer.horizon$setAbilities(player.getAbilities());
        local.setAbsorptionAmount(player.getAbsorptionAmount());
        local.experienceLevel = player.experienceLevel;
        local.experienceProgress = player.experienceProgress;
        local.totalExperience = player.totalExperience;
        local.tickCount = player.tickCount;
        local.invulnerableTime = player.invulnerableTime;
    }

    @SuppressWarnings("unchecked")
    private static java.util.Map<net.minecraft.world.effect.MobEffect, net.minecraft.world.effect.MobEffectInstance> effects(
            ServerPlayer player) {
        var map = new java.util.HashMap<net.minecraft.world.effect.MobEffect, net.minecraft.world.effect.MobEffectInstance>();
        for (var effect : player.getActiveEffects()) map.put(effect.getEffect(), effect);
        return map;
    }

    /**
     * The client's frame timer. A HUD frame here is one a tick, so each is a tick's worth of
     * time (`getDeltaFrameTime`: what fades take as time: Jade's tooltip fading in).
     */
    private static net.minecraft.client.Timer TIMER;

    /** The client's in-game GUI, made once without its constructor. */
    private static Gui gui(Minecraft minecraft) {
        if (gui == null) {
            gui = Allocate.make(Gui.class);
            var access = (dev.horizonlab.platform120.mixin.GuiAccess) gui;
            access.horizon$setMinecraft(minecraft);
            access.horizon$setRandom(net.minecraft.util.RandomSource.create());
            access.horizon$setItemRenderer(minecraft.getItemRenderer());
            // No boss bars: what overlays that keep clear of them ask (Jade's).
            access.horizon$setBossOverlay(new net.minecraft.client.gui.components.BossHealthOverlay(minecraft));
            // An empty chat, never focused (Jade hides while the chat is open).
            access.horizon$setChat(new net.minecraft.client.gui.components.ChatComponent(minecraft));
            // The player list, never shown (Jade hides while it is).
            access.horizon$setTabList(new net.minecraft.client.gui.components.PlayerTabOverlay(minecraft, gui));
            var client = (dev.horizonlab.platform120.mixin.MinecraftAccess) minecraft;
            client.horizon$setProfiler(net.minecraft.util.profiling.InactiveProfiler.INSTANCE);
            // A frame timer at 20 ticks a second (Minecraft's constructor makes it so).
            TIMER = new net.minecraft.client.Timer(20f, 0L);
            client.horizon$setTimer(TIMER);
            if (minecraft.gui == null) client.horizon$setGui(gui);
        }
        return gui;
    }

    /** Each dimension's stand-in client level, by its server level; and the other way. */
    private static final java.util.Map<net.minecraft.server.level.ServerLevel, net.minecraft.client.multiplayer.ClientLevel> CLIENT_LEVELS =
            new java.util.IdentityHashMap<>();
    private static final java.util.Map<net.minecraft.world.level.Level, net.minecraft.world.level.Level> SERVER_LEVELS =
            java.util.Collections.synchronizedMap(new java.util.IdentityHashMap<>());

    /**
     * The client level a player's game would have for `server`'s dimension: made without its
     * constructor (no connection), client-side, and reading `server`'s blocks, block entities,
     * entities, light, biomes (LevelMixin) and registries, which are
     * Horizon's. What client mods ask of the world they see (Jade's look at a block, a
     * minimap's chunks) is answered from there.
     */
    static synchronized net.minecraft.client.multiplayer.ClientLevel clientLevel(net.minecraft.server.level.ServerLevel server) {
        var made = CLIENT_LEVELS.get(server);
        if (made != null) return made;
        var level = Allocate.make(net.minecraft.client.multiplayer.ClientLevel.class);
        var data = Allocate.make(net.minecraft.client.multiplayer.ClientLevel.ClientLevelData.class);
        ((dev.horizonlab.platform120.mixin.ClientLevelAccess) level).horizon$setLevelData(data);
        var access = (dev.horizonlab.platform120.mixin.LevelAccess) level;
        access.horizon$setClientSide(true);
        access.horizon$setDimension(server.dimension());
        access.horizon$setDimensionType(server.dimensionTypeRegistration());
        access.horizon$setRandom(net.minecraft.util.RandomSource.create());
        access.horizon$setThreadSafeRandom(net.minecraft.util.RandomSource.createThreadSafe());
        access.horizon$setWorldBorder(server.getWorldBorder());
        access.horizon$setLevelData(data);
        access.horizon$setProfiler(() -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE);
        access.horizon$setDamageSources(server.damageSources());
        access.horizon$setRegistryAccess(server.registryAccess());
        // Its players: the client player, once one stands in (Screens120.clientPlayer).
        ((dev.horizonlab.platform120.mixin.ClientLevelAccess) level).horizon$setPlayers(new java.util.ArrayList<>());
        SERVER_LEVELS.put(level, server);
        CLIENT_LEVELS.put(server, level);
        return level;
    }

    /** The server level a stand-in client level reads, or null for any other level. */
    public static net.minecraft.world.level.Level serverFor(net.minecraft.world.level.Level level) {
        return level instanceof net.minecraft.client.multiplayer.ClientLevel ? SERVER_LEVELS.get(level) : null;
    }

    /**
     * One client tick for the HUD's player: what the crosshair hits (GameRenderer.pick, at the
     * reach of the player's mode) as `hitResult`, then Fabric's START and END client tick events.
     */
    private static void clientTick(Minecraft minecraft, ServerPlayer player, net.minecraft.client.player.LocalPlayer local,
            JsonObject state) {
        place(player, local);
        // The client's game mode, made without its connection: the player's mode, which its
        // reach (getPickRange) and the mods ask.
        if (minecraft.gameMode == null) {
            minecraft.gameMode = Allocate.make(net.minecraft.client.multiplayer.MultiPlayerGameMode.class);
            var made = (dev.horizonlab.platform120.mixin.MultiPlayerGameModeAccess) minecraft.gameMode;
            made.horizon$setMinecraft(minecraft);
            // What its constructor's field initializers set: no block being broken.
            made.horizon$setDestroyBlockPos(new net.minecraft.core.BlockPos(-1, -1, -1));
            made.horizon$setDestroyingItem(net.minecraft.world.item.ItemStack.EMPTY);
        }
        // The game renderer's camera at the player's eyes, as the frame before sets it up
        // (Jade traces from it). Render120 makes the renderer the same way when it comes first.
        if (minecraft.gameRenderer == null) {
            var renderer = Allocate.make(net.minecraft.client.renderer.GameRenderer.class);
            ((dev.horizonlab.platform120.mixin.GameRendererAccess) renderer).horizon$setMainCamera(new net.minecraft.client.Camera());
            ((dev.horizonlab.platform120.mixin.MinecraftAccess) minecraft).horizon$setGameRenderer(renderer);
        }
        minecraft.gameRenderer.getMainCamera().setup(minecraft.level, local, false, false, 1f);
        var mode = (dev.horizonlab.platform120.mixin.MultiPlayerGameModeAccess) minecraft.gameMode;
        mode.horizon$setMode(player.gameMode.getGameModeForPlayer());
        mode.horizon$setPreviousMode(player.gameMode.getPreviousGameModeForPlayer());
        double reach = player.getAbilities().instabuild ? 5.0 : 4.5;
        var block = player.pick(reach, 1f, false);
        var eye = player.getEyePosition(1f);
        var look = player.getViewVector(1f);
        double far = block.getType() == net.minecraft.world.phys.HitResult.Type.MISS ? reach : block.getLocation().distanceTo(eye);
        var end = eye.add(look.scale(far));
        var box = player.getBoundingBox().expandTowards(look.scale(far)).inflate(1.0);
        var entity = net.minecraft.world.entity.projectile.ProjectileUtil.getEntityHitResult(player, eye, end, box,
                e -> !e.isSpectator() && e.isPickable() && e != player, far * far);
        // Horizon's own mob under the crosshair (the runtime's level holds only the mods'):
        // a stand-in of its type where it is, with its health and name.
        var horizon = state.has("target") && state.get("target").isJsonObject()
                ? standIn(player.serverLevel(), state.getAsJsonObject("target")) : null;
        if (horizon != null) {
            entity = new net.minecraft.world.phys.EntityHitResult(horizon,
                    horizon.getBoundingBox().clip(eye, eye.add(look.scale(reach))).orElse(horizon.position()));
        }
        minecraft.hitResult = entity != null ? entity : block;
        minecraft.crosshairPickEntity = entity != null ? entity.getEntity() : null;
        if (DEBUG) {
            System.out.println("Hud120: client tick, looking at " + minecraft.hitResult.getType() + " "
                    + minecraft.hitResult.getLocation() + ", tick listeners "
                    + Events120.listeners(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK));
        }
        if (Events120.listeners(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.START_CLIENT_TICK) != 0
                || Events120.listeners(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK) != 0) {
            Events120.fire(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.START_CLIENT_TICK,
                    "client tick start", l -> l.onStartTick(minecraft));
            Events120.fire(net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK,
                    "client tick end", l -> l.onEndTick(minecraft));
        }
    }

    /** Stand-ins of Horizon's mobs, by Horizon's entity id: kept while one is looked at. */
    private static final java.util.Map<Integer, net.minecraft.world.entity.Entity> STAND_INS = new java.util.HashMap<>();
    /** The same stand-ins by their own id in the runtime. */
    private static final java.util.Map<Integer, net.minecraft.world.entity.Entity> BY_OWN_ID =
            new java.util.concurrent.ConcurrentHashMap<>();

    /** A stand-in of Horizon's mob by its id in the runtime, or null. */
    public static net.minecraft.world.entity.Entity standIn(int ownId) {
        return BY_OWN_ID.get(ownId);
    }

    /**
     * A stand-in for Horizon's mob `target` ({id, type, pos, yaw, head_yaw, health, max_health,
     * name}): an entity of its type, made once and not added to the level, where the mob is
     * and as hurt as it is. Null for a type the runtime doesn't know.
     */
    private static net.minecraft.world.entity.Entity standIn(net.minecraft.server.level.ServerLevel level, JsonObject target) {
        int id = target.get("id").getAsInt();
        String type = target.get("type").getAsString();
        var made = STAND_INS.get(id);
        if (made == null || made.level() != level
                || !net.minecraft.world.entity.EntityType.getKey(made.getType()).toString().equals(type)) {
            if (STAND_INS.size() > 64) {
                STAND_INS.clear();
                BY_OWN_ID.clear();
            }
            var kind = net.minecraft.world.entity.EntityType.byString(type).orElse(null);
            if (kind == null) return null;
            made = kind.create(level);
            if (made == null) return null;
            STAND_INS.put(id, made);
            BY_OWN_ID.put(made.getId(), made);
        }
        var pos = target.getAsJsonArray("pos");
        float yaw = target.has("yaw") ? target.get("yaw").getAsFloat() : 0f;
        made.moveTo(pos.get(0).getAsDouble(), pos.get(1).getAsDouble(), pos.get(2).getAsDouble(), yaw, 0f);
        if (target.has("head_yaw")) made.setYHeadRot(target.get("head_yaw").getAsFloat());
        if (made instanceof net.minecraft.world.entity.LivingEntity living) {
            var max = living.getAttribute(net.minecraft.world.entity.ai.attributes.Attributes.MAX_HEALTH);
            if (max != null && target.has("max_health")) max.setBaseValue(target.get("max_health").getAsDouble());
            if (target.has("health")) living.setHealth(target.get("health").getAsFloat());
        }
        if (target.has("name") && target.get("name").isJsonPrimitive()) {
            made.setCustomName(net.minecraft.network.chat.Component.literal(target.get("name").getAsString()));
        }
        return made;
    }

    /**
     * The client player where the server player is, looking where it looks: its position, block
     * and chunk, box, size and rotation (what client mods read of `Minecraft.player`).
     */
    private static void place(ServerPlayer player, net.minecraft.client.player.LocalPlayer local) {
        var entity = (dev.horizonlab.platform120.mixin.EntityAccess) (Object) local;
        entity.horizon$setPosition(player.position());
        entity.horizon$setBlockPosition(player.blockPosition());
        entity.horizon$setChunkPosition(player.chunkPosition());
        entity.horizon$setBoundingBox(player.getBoundingBox());
        entity.horizon$setDimensions(player.getDimensions(player.getPose()));
        entity.horizon$setEyeHeight(player.getEyeHeight());
        local.setYRot(player.getYRot());
        local.setXRot(player.getXRot());
        local.yRotO = player.yRotO;
        local.xRotO = player.xRotO;
        local.xo = player.xo;
        local.yo = player.yo;
        local.zo = player.zo;
    }

    /** An empty client level, for what the HUD asks of the world (hardcore hearts). */
    private static net.minecraft.client.multiplayer.ClientLevel level() {
        if (level == null) {
            level = Allocate.make(net.minecraft.client.multiplayer.ClientLevel.class);
            ((dev.horizonlab.platform120.mixin.ClientLevelAccess) level).horizon$setLevelData(
                    Allocate.make(net.minecraft.client.multiplayer.ClientLevel.ClientLevelData.class));
        }
        return level;
    }

    private interface Part {
        void run() throws Throwable;
    }

    /** One part of the HUD; one that throws is left out of this frame, and said. */
    private static void part(String name, Part part) {
        try {
            part.run();
        } catch (Throwable t) {
            failed(name, t);
        }
    }

    private static void failed(String what, Throwable t) {
        Throwable cause = t instanceof java.lang.reflect.InvocationTargetException e && e.getCause() != null ? e.getCause() : t;
        if (FAILURES.merge(what, 1, Integer::sum) <= 3) {
            System.out.println("Hud120: the HUD's " + what + " failed:");
            cause.printStackTrace(System.out);
        }
    }

    /**
     * Every handler merged into `Gui.render(GuiGraphics, float)` taking its arguments and a
     * CallbackInfo: Fabric's, which fires HudRenderCallback, and the mods' own.
     */
    private static void renderHandlers(Gui gui, GuiGraphics graphics) {
        for (var m : Gui.class.getDeclaredMethods()) {
            MixinMerged merged = m.getAnnotation(MixinMerged.class);
            if (merged == null || merged.mixin().startsWith("dev.horizonlab.")) continue;
            Class<?>[] params = m.getParameterTypes();
            if (params.length != 3 || params[0] != GuiGraphics.class || params[1] != float.class
                    || !CallbackInfo.class.isAssignableFrom(params[2])) {
                continue;
            }
            part(merged.mixin() + "." + m.getName(), () -> {
                m.setAccessible(true);
                m.invoke(java.lang.reflect.Modifier.isStatic(m.getModifiers()) ? null : gui,
                        graphics, 0f, new CallbackInfo("render", true));
            });
        }
    }
}
