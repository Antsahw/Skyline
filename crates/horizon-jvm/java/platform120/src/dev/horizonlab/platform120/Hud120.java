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
        minecraft.level = level();
        minecraft.cameraEntity = local;
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
            return recording.finish().toString();
        } finally {
            if (Graphics120.current() == recording) recording.finish();
            minecraft.level = levelBefore;
            minecraft.cameraEntity = cameraBefore;
        }
    }

    /** Horizon's numbers on the runtime's player, which the client player reads. */
    private static void stats(ServerPlayer player, JsonObject s) {
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
        entity.horizon$setLevel(player.level());
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

    /** The client's in-game GUI, made once without its constructor. */
    private static Gui gui(Minecraft minecraft) {
        if (gui == null) {
            gui = Allocate.make(Gui.class);
            var access = (dev.horizonlab.platform120.mixin.GuiAccess) gui;
            access.horizon$setMinecraft(minecraft);
            access.horizon$setRandom(net.minecraft.util.RandomSource.create());
            access.horizon$setItemRenderer(minecraft.getItemRenderer());
            var client = (dev.horizonlab.platform120.mixin.MinecraftAccess) minecraft;
            client.horizon$setProfiler(net.minecraft.util.profiling.InactiveProfiler.INSTANCE);
            // A frame timer at 20 ticks a second (Minecraft's constructor makes it so).
            client.horizon$setTimer(new net.minecraft.client.Timer(20f, 0L));
            if (minecraft.gui == null) client.horizon$setGui(gui);
        }
        return gui;
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
