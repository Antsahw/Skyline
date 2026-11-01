package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * The second round of vanilla actions Horizon does in Rust that mods patch (the mixin table on
 * a 91-jar pack): a player's attack on and use of Horizon's own entities, the item used in the
 * air, and a crafting result taken. Horizon's objects aren't in the mirror, so each call makes
 * a copy for it (the entity from Horizon's NBT, the stack, the crafting grid), runs what the
 * mods hook on it, and sends back what they did: stopped, took the click, removed the entity
 * (Carry On's pickup), changed the player's slots.
 */
public final class HorizonVanillaActions {
    private HorizonVanillaActions() {}

    /** HORIZON_JAVA_DEBUG=entities: each attack on and use of Horizon's entities is logged. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("entities");

    // ---- Horizon's entities: attack and use ----------------------------------------------

    private static final String ATTACK = "(Lnet/minecraft/world/entity/Entity;)V";
    private static final String INTERACT_ON =
            "(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/phys/Vec3;)Lnet/minecraft/world/InteractionResult;";

    /**
     * A player attacks ({@code use} false) or uses one of Horizon's entities, given as vanilla
     * saves it ({@code snbt}): Fabric's AttackEntityCallback / UseEntityCallback (Carry On's
     * pickup, Mob Lassos, claims), then the mods' injections at the head of Player.attack /
     * Player.interactOn, on a copy of the entity made for the call. Bits: 1 a mod took it (Horizon
     * does nothing more), 2 the arm swings, 4 the mods removed the entity (picked up: Horizon
     * removes its own, without drops). The copy is never added to a level.
     */
    public static int entity(String uuid, String dimension, String snbt, boolean use, int hand,
                             double x, double y, double z, int selected) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null || snbt == null) return 0;
        ServerLevel level = HorizonServer.level(dimension);
        if (selected >= 0 && selected < 9) player.getInventory().setSelectedSlot(selected);
        Entity target;
        try {
            var tag = net.minecraft.nbt.TagParser.parseCompoundFully(snbt);
            target = EntityType.loadEntityRecursive(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), tag),
                    level, EntitySpawnReason.LOAD, e -> e);
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("vanilla entity " + t.getClass())) Natives.log("Java mods: an entity Horizon has couldn't be copied: " + t);
            return 0;
        }
        if (target == null) return 0;
        var interaction = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
        int[] out = {0};
        if (!use) {
            HorizonServer.fireEach("AttackEntityCallback", net.fabricmc.fabric.api.event.player.AttackEntityCallback.EVENT, l -> {
                if (out[0] != 0) return;
                out[0] = bits(l.interact(player, level, interaction, target, null));
            });
            // A routed attack (HorizonCombat) runs Player.attack itself, its head included.
            if (out[0] == 0 && !HorizonCombat.routed() && HorizonHooks.hooked("net/minecraft/world/entity/player/Player", "attack")) {
                var r = HorizonHooks.fire(player, Player.class, "attack", ATTACK, "HEAD", new Object[] {target}, null, true);
                if (r.cancelled()) out[0] = 1;
            }
        } else {
            var hit = new EntityHitResult(target, new Vec3(x, y, z));
            HorizonServer.fireEach("UseEntityCallback", net.fabricmc.fabric.api.event.player.UseEntityCallback.EVENT, l -> {
                if (out[0] != 0) return;
                out[0] = bits(l.interact(player, level, interaction, target, hit));
            });
            if (out[0] == 0 && HorizonHooks.hooked("net/minecraft/world/entity/player/Player", "interactOn")) {
                Object[] args = {target, interaction, new Vec3(x - target.getX(), y - target.getY(), z - target.getZ())};
                var r = HorizonHooks.fire(player, Player.class, "interactOn", INTERACT_ON, "HEAD", args, InteractionResult.PASS, true);
                if (r.cancelled()) out[0] = r.returnValue() instanceof InteractionResult res ? Math.max(1, bits(res)) : 1;
            }
        }
        if (target.isRemoved()) out[0] |= 4 | 1;
        if (DEBUG) Natives.log("entities: " + (use ? "use of " : "attack on ") + target + " by " + player.getName().getString()
                + " (sneaking " + player.isShiftKeyDown() + "): " + out[0]);
        HorizonServer.afterCall();
        return out[0];
    }

    private static int bits(InteractionResult r) {
        if (r == null || r == InteractionResult.PASS) return 0;
        return r instanceof InteractionResult.Success s && s.shouldSwing() ? 3 : 1;
    }

    /**
     * A vanilla entity a mod's code adds to a mirrored level (Carry On putting a carried cow
     * down): saved as vanilla saves it, for Horizon to load as its own. True when sent.
     */
    static boolean spawnVanilla(ServerLevel level, Entity entity) {
        if (!(entity instanceof net.minecraft.world.entity.Mob)) return false;
        try {
            TagValueOutput out = TagValueOutput.createWithContext(ProblemReporter.DISCARDING, level.registryAccess());
            if (!entity.save(out)) return false;
            if (DEBUG) {
                Natives.log("entities: a mod added " + entity + " to Horizon's world");
                for (StackTraceElement f : new Throwable().getStackTrace()) Natives.log("    at " + f);
            }
            Natives.spawnEntity(HorizonServer.dimensionOf(level), out.buildResult().toString());
            return true;
        } catch (Throwable t) {
            Natives.log("Java mods: a vanilla entity a mod added couldn't go to Horizon: " + t);
            return false;
        }
    }

    // ---- An item used in the air ----------------------------------------------------------

    private static final String USE = "(Lnet/minecraft/world/level/Level;Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;";
    private static final Set<String> USE_METHODS = Set.of("use", "releaseUsing", "finishUsingItem", "getUseDuration", "getUseAnimation");
    private static final Set<String> USE_FIRED = Set.of("net/minecraft/world/item/Item.use", "net/minecraft/world/item/ItemStack.use");
    private static final Map<Class<?>, Boolean> USE_ROUTED = new ConcurrentHashMap<>();

    /**
     * Whether using this vanilla item in the air is the mirror's: a mod patches its class's use
     * (or what it calls) other than at a head Horizon fires (any class's use, ItemStack.use): a bucket
     * whose use claims mods patch (Architectury, Open Parties and Claims).
     */
    public static boolean useRouted(ItemStack stack) {
        return USE_ROUTED.computeIfAbsent(stack.getItem().getClass(), c -> {
            // The heads of use on the item's classes are fired (fireUse), not routed.
            Set<String> fired = new java.util.HashSet<>(USE_FIRED);
            for (Class<?> k = c; k != null && k != Object.class; k = k.getSuperclass()) fired.add(k.getName().replace('.', '/') + ".use");
            boolean routed = HorizonMixinTable.patchedInside(c, USE_METHODS, fired)
                    || HorizonMixinTable.patchedInside(ItemStack.class, Set.of("use"), USE_FIRED);
            if (routed) Natives.log("Using " + BuiltInRegistries.ITEM.getKey(stack.getItem()) + " in the air goes to the mods (they patch its inside)");
            return routed;
        });
    }

    /**
     * The mods' injections at the head of ItemStack.use and of Item.use on the item's class
     * chain (GlitchCore's item-use event, Naturalist's bottle, Trinkets' equip), for a vanilla
     * item Horizon uses itself. The result a handler set when one cancels; else null.
     */
    public static InteractionResult fireUse(ServerLevel level, ServerPlayer player, ItemStack stack, InteractionHand hand) {
        Object[] args = {level, player, hand};
        if (HorizonHooks.hooked("net/minecraft/world/item/ItemStack", "use")) {
            var r = HorizonHooks.fire(stack, ItemStack.class, "use", USE, "HEAD", args, InteractionResult.PASS, true);
            if (r.cancelled()) return r.returnValue() instanceof InteractionResult res ? res : InteractionResult.SUCCESS;
        }
        for (Class<?> c = stack.getItem().getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            String owner = c.getName().replace('.', '/');
            if (!HorizonHooks.hooked(owner, "use")) continue;
            var r = HorizonHooks.fire(stack.getItem(), c, "use", USE, "HEAD", args, InteractionResult.PASS, true);
            if (r.cancelled()) return r.returnValue() instanceof InteractionResult res ? res : InteractionResult.SUCCESS;
        }
        return null;
    }

    // ---- A crafting result taken ----------------------------------------------------------

    private static java.lang.reflect.Field removeCount;

    /**
     * A player took {@code count} of a crafting result ({@code result}, SNBT) made by recipe
     * {@code recipe} from a grid of {@code width} ({@code grid}: SNBT, "" for empty, as it was
     * before the take): vanilla's ResultSlot.onTake on copies (Architectury's and Balm's
     * crafted events, Modonomicon's and RebornCore's hooks in checkTakeAchievements, the
     * item's onCraftedBy). The grid copy's changes are dropped (Horizon took the ingredients);
     * what the mods give the player comes back as slots.
     */
    public static void crafted(String uuid, String recipe, String result, int count, int width, String[] grid) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        ItemStack stack = HorizonServer.item(result);
        if (DEBUG) Natives.log("crafting: " + uuid + " took " + count + " of " + result + " (" + recipe + ")");
        if (player == null || stack.isEmpty() || grid == null || width <= 0) return;
        HorizonServer.fire("ResultSlot.onTake", () -> {
            var menu = player.inventoryMenu;
            var craftSlots = new net.minecraft.world.inventory.TransientCraftingContainer(menu, width, Math.max(1, grid.length / width));
            // One of each ingredient: onTake takes one from each cell, so a remainder (a milk
            // bucket's bucket) goes back into the copy's cell, never into the player's
            // inventory (Horizon has given it already).
            for (int i = 0; i < grid.length && i < craftSlots.getContainerSize(); i++) {
                ItemStack cell = HorizonServer.item(grid[i]);
                craftSlots.setItem(i, cell.isEmpty() ? cell : cell.copyWithCount(1));
            }
            var results = new net.minecraft.world.inventory.ResultContainer();
            try {
                var key = ResourceKey.create(Registries.RECIPE, Identifier.parse(recipe));
                player.level().getServer().getRecipeManager().byKey(key).ifPresent(results::setRecipeUsed);
            } catch (RuntimeException e) {
                // No such recipe in the mirror: the take without it.
            }
            var slot = new net.minecraft.world.inventory.ResultSlot(player, craftSlots, results, 0, 0, 0);
            try {
                if (removeCount == null) {
                    removeCount = net.minecraft.world.inventory.ResultSlot.class.getDeclaredField("removeCount");
                    removeCount.setAccessible(true);
                }
                removeCount.setInt(slot, count);
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException(e);
            }
            slot.onTake(player, stack.copyWithCount(Math.max(1, count)));
            if (DEBUG) Natives.log("crafting: the mirror's inventory after: " + player.getInventory().getNonEquipmentItems());
        });
        HorizonServer.afterCall();
    }

    // ---- The hotbar, riding --------------------------------------------------------------

    /**
     * A player picks hotbar slot {@code slot} (ServerGamePacketListenerImpl.handleSetCarriedItem:
     * Inventory.setSelectedSlot): the mods' injections at its head (Carry On keeps the slot
     * while carrying). True when one cancelled: Horizon keeps the old slot and tells the game.
     */
    public static boolean selectSlot(String uuid, int slot) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        String owner = "net/minecraft/world/entity/player/Inventory";
        if (player == null || !HorizonHooks.hooked(owner, "setSelectedSlot")) return false;
        var r = HorizonHooks.fire(player.getInventory(), net.minecraft.world.entity.player.Inventory.class, "setSelectedSlot", "(I)V",
                "HEAD", new Object[] {slot}, null, true);
        HorizonServer.afterCall();
        return r.cancelled();
    }

    private static final String START_RIDING = "(Lnet/minecraft/world/entity/Entity;ZZ)Z";

    /**
     * A player got onto one of Horizon's vehicles ({@code type} at x y z, a mob's NBT when it
     * is one): the mods' injections at the head of Entity.startRiding on the mirrored player,
     * with a copy of the vehicle. True when one cancelled (Horizon takes the player off).
     */
    public static boolean ride(String uuid, String dimension, String type, double x, double y, double z, String snbt) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null || !HorizonHooks.hooked("net/minecraft/world/entity/Entity", "startRiding")) return false;
        ServerLevel level = HorizonServer.level(dimension);
        Entity vehicle = vehicleCopy(level, type, x, y, z, snbt);
        if (vehicle == null) return false;
        var r = HorizonHooks.fire(player, Entity.class, "startRiding", START_RIDING, "HEAD", new Object[] {vehicle, false, true},
                Boolean.TRUE, true);
        HorizonServer.afterCall();
        if (DEBUG) Natives.log("entities: " + player.getName().getString() + " rides " + vehicle + ": " + (r.cancelled() ? "stopped" : "ok"));
        return r.cancelled() && !Boolean.TRUE.equals(r.returnValue());
    }

    /** A copy of Horizon's vehicle: a mob from its NBT, else a new entity of the type there. */
    private static Entity vehicleCopy(ServerLevel level, String type, double x, double y, double z, String snbt) {
        try {
            if (snbt != null && !snbt.isEmpty()) {
                var tag = net.minecraft.nbt.TagParser.parseCompoundFully(snbt);
                return EntityType.loadEntityRecursive(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), tag),
                        level, EntitySpawnReason.LOAD, e -> e);
            }
            var entityType = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getValue(net.minecraft.resources.Identifier.parse(type));
            Entity vehicle = entityType == null ? null : entityType.create(level, EntitySpawnReason.LOAD);
            if (vehicle != null) vehicle.setPos(x, y, z);
            return vehicle;
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("vehicle " + type)) Natives.log("Java mods: a vehicle (" + type + ") couldn't be copied: " + t);
            return null;
        }
    }

    private static java.lang.reflect.Field vehicleField;

    /** Sets a mirrored player's vehicle for a call (it never rides on the mirror). */
    private static void setVehicle(Entity rider, Entity vehicle) {
        try {
            if (vehicleField == null) {
                vehicleField = Entity.class.getDeclaredField("vehicle");
                vehicleField.setAccessible(true);
            }
            vehicleField.set(rider, vehicle);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    /**
     * A player gets off Horizon's vehicle (sneaking: Player.rideTick's stopRiding, which calls
     * removeVehicle). Before Horizon does ({@code after} false): the mods' injections at the
     * heads of LivingEntity/Entity.stopRiding and Player/Entity.removeVehicle, the player sitting
     * in a copy of the vehicle; true when one cancelled (it stays on). After it did: their
     * ends, in vanilla's order back out.
     */
    public static boolean dismount(String uuid, String dimension, String type, double x, double y, double z, String snbt,
                                   boolean after) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        String entity = "net/minecraft/world/entity/Entity", living = "net/minecraft/world/entity/LivingEntity",
                playerClass = "net/minecraft/world/entity/player/Player";
        if (player == null || !HorizonHooks.hooked(entity, "stopRiding") && !HorizonHooks.hooked(living, "stopRiding")
                && !HorizonHooks.hooked(entity, "removeVehicle") && !HorizonHooks.hooked(playerClass, "removeVehicle")) {
            return false;
        }
        ServerLevel level = HorizonServer.level(dimension);
        Entity vehicle = vehicleCopy(level, type, x, y, z, snbt);
        if (vehicle == null) return false;
        Object[] none = {};
        boolean cancelled = false;
        // The rider sits in it before (the heads see getVehicle()); after, it's off.
        if (!after) setVehicle(player, vehicle);
        try {
            if (!after) {
                Object[][] chain = {{net.minecraft.world.entity.LivingEntity.class, "stopRiding"}, {Entity.class, "stopRiding"},
                        {Player.class, "removeVehicle"}, {Entity.class, "removeVehicle"}};
                for (Object[] c : chain) {
                    Class<?> owner = (Class<?>) c[0];
                    if (!HorizonHooks.hooked(owner.getName().replace('.', '/'), (String) c[1])) continue;
                    if (HorizonHooks.fire(player, owner, (String) c[1], "()V", "HEAD", none, null, true).cancelled()) {
                        cancelled = true;
                        break;
                    }
                }
            } else {
                Object[][] chain = {{Entity.class, "removeVehicle"}, {Player.class, "removeVehicle"}, {Entity.class, "stopRiding"},
                        {net.minecraft.world.entity.LivingEntity.class, "stopRiding"}};
                for (Object[] c : chain) {
                    Class<?> owner = (Class<?>) c[0];
                    if (!HorizonHooks.hooked(owner.getName().replace('.', '/'), (String) c[1])) continue;
                    for (String at : new String[] {"RETURN", "TAIL"}) HorizonHooks.fire(player, owner, (String) c[1], "()V", at, none, null, true);
                }
            }
        } finally {
            setVehicle(player, null);
        }
        HorizonServer.afterCall();
        if (DEBUG) Natives.log("entities: " + player.getName().getString() + (after ? " got off " : " gets off ") + vehicle
                + " holding " + player.getMainHandItem() + (cancelled ? ": stopped" : ""));
        return cancelled;
    }

    // ---- Effects ---------------------------------------------------------------------------

    /**
     * Horizon adds effect {@code id} to a player (potions, food, totems, /effect): the mods'
     * injections at the head of LivingEntity.addEffect, with the instance as vanilla would
     * make it; true when one cancelled (Spell Engine's immunities, Open Parties and Claims'
     * protection).
     */
    public static boolean addEffect(String uuid, String id, int duration, int amplifier, boolean ambient, boolean visible) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        String owner = "net/minecraft/world/entity/LivingEntity";
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null || !HorizonHooks.hooked(owner, "addEffect")) return false;
        var effect = BuiltInRegistries.MOB_EFFECT.get(Identifier.parse(id));
        if (effect.isEmpty()) return false;
        var instance = new net.minecraft.world.effect.MobEffectInstance(effect.get(), duration, amplifier, ambient, visible);
        boolean cancelled = HorizonHooks.fire(player, LivingEntity.class, "addEffect",
                "(Lnet/minecraft/world/effect/MobEffectInstance;Lnet/minecraft/world/entity/Entity;)Z", "HEAD",
                new Object[] {instance, null}, null, true).cancelled();
        HorizonServer.afterCall();
        if (DEBUG) Natives.log("entities: " + player.getName().getString() + " gets " + instance + (cancelled ? ": stopped" : ""));
        return cancelled;
    }

    // ---- Fall damage --------------------------------------------------------------------

    private static Boolean fallRouted;
    /** The player whose routed fall runs now, with Horizon's {safe distance, damage multiplier, immune}. */
    private static LivingEntity falling;
    private static double[] fallAttributes;
    private static float fallCaught;

    /**
     * Whether a player's fall damage goes to the mirror: a mod patches causeFallDamage or
     * calculateFallDamage (Moving Elevators, Mythic Upgrades, Balm's and Puzzles Lib's events).
     */
    public static boolean fallRouted() {
        if (fallRouted == null) {
            Set<String> methods = Set.of("causeFallDamage", "calculateFallDamage");
            fallRouted = HorizonMixinTable.patchedInside(ServerPlayer.class, methods, Set.of());
            if (fallRouted) Natives.log("Fall damage goes to the mods (" + HorizonMixinTable.mods("net/minecraft/world/entity/LivingEntity", "causeFallDamage")
                    + HorizonMixinTable.mods("net/minecraft/world/entity/LivingEntity", "calculateFallDamage") + ")");
        }
        return fallRouted;
    }

    /**
     * A player lands: vanilla's {@code causeFallDamage(distance, multiplier, source)} runs on
     * the mirrored player with the mods' patches. calculateFallDamage answers with Horizon's
     * safe fall distance, damage multiplier and immunity (the mirror doesn't keep effects and
     * attributes); what reaches hurtServer is caught, not applied (FallCaptureMixin). The
     * damage Horizon deals (0: none).
     */
    public static float fall(String uuid, String dimension, double distance, float multiplier, boolean stalagmite,
                             double safe, double damageMultiplier, boolean immune) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null) return -1;
        ServerLevel level = HorizonServer.level(dimension);
        var sources = level.damageSources();
        fallAttributes = new double[] {safe, damageMultiplier, immune ? 1 : 0};
        fallCaught = 0;
        falling = player;
        try {
            player.causeFallDamage(distance, multiplier, stalagmite ? sources.stalagmite() : sources.fall());
        } finally {
            falling = null;
        }
        HorizonServer.afterCall();
        if (DEBUG) Natives.log("entities: " + player.getName().getString() + " falls " + distance + " (x" + multiplier + "): " + fallCaught);
        return fallCaught;
    }

    /** Whether {@code entity} is the player in a routed fall (FallCaptureMixin). */
    public static boolean falling(Object entity) {
        return entity == falling;
    }

    /** LivingEntity.calculateFallDamage with Horizon's attributes, for the routed fall. */
    public static int fallDamage(double distance, float multiplier) {
        if (fallAttributes[2] != 0) return 0;
        return (int) Math.floor((distance + 1.0E-6 - fallAttributes[0]) * multiplier * fallAttributes[1]);
    }

    /** The routed fall's damage reached hurtServer. */
    public static void caughtFall(float amount) {
        fallCaught += amount;
    }

    // ---- Precipitation: biome weather and snow --------------------------------------------

    private static final Set<String> WEATHER = Set.of("shouldSnow", "shouldFreeze", "getPrecipitationAt", "coldEnoughToSnow",
            "warmEnoughToRain", "getTemperature", "getHeightAdjustedTemperature");
    private static Boolean weatherRouted;

    /**
     * Whether Horizon's precipitation ticks (ServerLevel.tickPrecipitation: ice, snow layers,
     * cauldrons) ask the mirror: a mod patches the biome's weather answers or the tick itself
     * (Serene Seasons' seasonal snow). Performance mods (Lithium) don't count.
     */
    public static boolean weatherRouted() {
        if (weatherRouted == null) {
            weatherRouted = HorizonMixinTable.patchedInside(net.minecraft.world.level.biome.Biome.class, WEATHER, Set.of())
                    || HorizonMixinTable.patchedInside(ServerLevel.class, Set.of("tickPrecipitation"), Set.of());
            if (weatherRouted) Natives.log("Precipitation goes to the mods (" + HorizonMixinTable.mods("net/minecraft/world/level/biome/Biome", "shouldSnow")
                    + HorizonMixinTable.mods("net/minecraft/server/level/ServerLevel", "tickPrecipitation") + ")");
        }
        return weatherRouted;
    }

    /**
     * Precipitation ticks of {@code dimension}: each column's top (motion-blocking: x, y, z
     * per column, as Horizon found it) answered as tickPrecipitation asks the biome there:
     * shouldFreeze below the top, shouldSnow at it, getPrecipitationAt below it (0 none, 1
     * rain, 2 snow). Per column {freezes, snows, kind}.
     */
    public static int[] precipitation(String dimension, int[] tops) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerLevel level = HorizonServer.level(dimension);
        int[] out = new int[tops.length];
        int sea = level.getSeaLevel();
        for (int i = 0; i + 2 < tops.length; i += 3) {
            var top = new net.minecraft.core.BlockPos(tops[i], tops[i + 1], tops[i + 2]);
            var below = top.below();
            int at = i;
            HorizonServer.fire("ServerLevel.tickPrecipitation", () -> {
                var biome = level.getBiome(top).value();
                out[at] = biome.shouldFreeze(level, below) ? 1 : 0;
                out[at + 1] = biome.shouldSnow(level, top) ? 1 : 0;
                var kind = biome.getPrecipitationAt(below, sea);
                out[at + 2] = kind == net.minecraft.world.level.biome.Biome.Precipitation.RAIN ? 1
                        : kind == net.minecraft.world.level.biome.Biome.Precipitation.SNOW ? 2 : 0;
            });
        }
        return out;
    }
}
