package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.storage.TagValueInput;

/**
 * A player's attack on Horizon's own mobs, routed: vanilla's Player.attack runs on the mirror,
 * with the mods' patches inside it (Better Combat's combo damage and knockback, Farmer's
 * Delight's knives, Spell Engine), against copies of the mobs made from Horizon's NBT and
 * standing in the mirror's level for the call (so a sweep, or Better Combat's targets, find the
 * ones around). What reaches each copy's hurtServer and knockback is caught (LivingEntityMixin:
 * vanilla's body doesn't run on a copy) and sent back: Horizon hurts its own mobs with it.
 *
 * <p>With Better Combat, a click is its attack request (Horizon's game sends vanilla's attack,
 * Better Combat's client half doesn't run): the combo counted here, its targets found by its
 * own TargetFinder among the copies, its server handler (ServerNetwork.handleAttackRequest)
 * run as for its packet.
 */
public final class HorizonCombat {
    private HorizonCombat() {}

    /** HORIZON_JAVA_DEBUG=combat: each routed attack and what it caught. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("combat");

    /** The copies standing in each level for the call (ServerLevel.getEntities sees them). */
    private static final Map<ServerLevel, List<Entity>> VISITING = new IdentityHashMap<>();
    /** What each copy took during the call, by copy: damage, knockback. */
    private static final Map<Entity, Hit> CAUGHT = new IdentityHashMap<>();

    /** One copy's share of an attack: the damage hurtServer got, the knockbacks. */
    static final class Hit {
        float damage;
        String damageType = "";
        double knockStrength, knockX, knockZ;
    }

    /** The copies visiting a level now (HorizonServer.entities). */
    public static List<Entity> visiting(ServerLevel level) {
        List<Entity> v = VISITING.get(level);
        return v == null ? List.of() : v;
    }

    /** Whether {@code entity} is a copy of Horizon's, standing in for a routed attack. */
    public static boolean isCopy(Object entity) {
        return entity instanceof Entity e && CAUGHT.containsKey(e);
    }

    /** LivingEntity.hurtServer on a copy (LivingEntityMixin): the damage, caught. */
    public static void caughtHurt(Entity copy, net.minecraft.world.damagesource.DamageSource source, float amount) {
        Hit hit = CAUGHT.get(copy);
        if (hit == null) return;
        hit.damage += amount;
        if (hit.damageType.isEmpty()) {
            hit.damageType = source.typeHolder().unwrapKey().map(k -> k.identifier().toString()).orElse("minecraft:player_attack");
        }
    }

    /** LivingEntity.knockback on a copy (LivingEntityMixin): strength and direction, caught. */
    public static void caughtKnockback(Entity copy, double strength, double x, double z) {
        Hit hit = CAUGHT.get(copy);
        if (hit == null) return;
        hit.knockStrength += strength;
        hit.knockX = x;
        hit.knockZ = z;
    }

    private static Boolean routed;

    /**
     * Whether a player's attack is the mirror's: Better Combat is loaded, or a mod patches
     * the inside of Player.attack (not only its head, which HorizonVanillaActions fires).
     */
    public static boolean routed() {
        if (routed == null) {
            routed = betterCombat() || HorizonMixinTable.patchedInside(Player.class, Set.of("attack"),
                    Set.of("net/minecraft/world/entity/player/Player.attack"));
            if (routed) {
                Natives.log("Players' attacks go to the mods (" + HorizonMixinTable.mods("net/minecraft/world/entity/player/Player", "attack")
                        + (betterCombat() ? ", Better Combat's attack requests" : "") + ")");
            }
        }
        return routed;
    }

    private static Boolean betterCombat;

    private static boolean betterCombat() {
        if (betterCombat == null) betterCombat = net.fabricmc.loader.api.FabricLoader.getInstance().isModLoaded("bettercombat");
        return betterCombat;
    }

    /** Per player: Better Combat's combo count and the tick of the last attack (its client keeps these). */
    private static final Map<UUID, long[]> COMBO = new ConcurrentHashMap<>();

    /**
     * Player {@code uuid} attacks Horizon's mob {@code ids[0]} (the one under the crosshair);
     * {@code nbts} are it and the mobs around (vanilla's NBT, ids {@code ids}), {@code ticks}
     * the player's ticks since its last swing (attack strength), {@code yaw}/{@code pitch}
     * where it looked. A flat array: per mob hit, {id, damage, knockback strength, x, z}; and a
     * damage type per hit in {@code types} (out, by index). Null when the mirror can't.
     */
    public static double[] attack(String uuid, String dimension, String[] nbts, int[] ids, int ticks, float yaw, float pitch,
                                  int selected, String[] types) {
        Thread.currentThread().setContextClassLoader(HorizonCombat.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null || nbts.length == 0) return null;
        ServerLevel level = HorizonServer.level(dimension);
        if (selected >= 0 && selected < 9) player.getInventory().setSelectedSlot(selected);
        player.setYRot(yaw);
        player.setXRot(pitch);
        player.setYHeadRot(yaw);
        Map<Entity, Integer> copies = new LinkedHashMap<>();
        for (int i = 0; i < nbts.length; i++) {
            try {
                var tag = net.minecraft.nbt.TagParser.parseCompoundFully(nbts[i]);
                Entity copy = EntityType.loadEntityRecursive(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), tag),
                        level, EntitySpawnReason.LOAD, e -> e);
                if (copy == null) continue;
                copy.setId(ids[i]);
                copies.put(copy, ids[i]);
            } catch (Throwable t) {
                if (HorizonServer.failedOnce("combat copy " + t.getClass())) Natives.log("Java mods: a mob couldn't be copied for an attack: " + t);
            }
        }
        if (copies.isEmpty()) return null;
        Entity target = copies.keySet().iterator().next();
        if (target instanceof LivingEntity && ids[0] != copies.get(target)) return null;
        for (Entity copy : copies.keySet()) CAUGHT.put(copy, new Hit());
        VISITING.put(level, new ArrayList<>(copies.keySet()));
        try {
            HorizonEntities.equipment(player);
            setStrength(player, ticks);
            if (DEBUG) Natives.log("combat: ticks " + ticks + ", the level finds the target: " + level.getEntity(ids[0])
                    + ", distance " + Math.sqrt(player.distanceToSqr(target)));
            boolean viaBetterCombat = betterCombat() && betterCombatAttack(player, level, target);
            if (!viaBetterCombat) HorizonServer.fire("Player.attack", () -> player.attack(target));
            HorizonServer.runTasks();
            HorizonServer.afterCall();
            List<Double> out = new ArrayList<>();
            int k = 0;
            for (var e : copies.entrySet()) {
                Hit hit = CAUGHT.get(e.getKey());
                if (hit == null || hit.damage <= 0 && hit.knockStrength <= 0) continue;
                out.add((double) e.getValue());
                out.add((double) hit.damage);
                out.add(hit.knockStrength);
                out.add(hit.knockX);
                out.add(hit.knockZ);
                if (k < types.length) types[k] = hit.damageType;
                k++;
            }
            if (DEBUG) Natives.log("combat: " + player.getName().getString() + " attacks " + target + (viaBetterCombat ? " (Better Combat)" : "")
                    + ": " + out);
            double[] result = new double[out.size()];
            for (int i = 0; i < result.length; i++) result[i] = out.get(i);
            return result;
        } finally {
            VISITING.remove(level);
            for (Entity copy : copies.keySet()) CAUGHT.remove(copy);
        }
    }

    /** The mirror's player isn't ticked: its attack strength is Horizon's swing (ticks since it). */
    private static void setStrength(ServerPlayer player, int ticks) {
        try {
            var ticker = LivingEntity.class.getDeclaredField("attackStrengthTicker");
            ticker.setAccessible(true);
            ticker.setInt(player, Math.max(0, ticks));
        } catch (ReflectiveOperationException e) {
            // Full strength, then.
        }
    }

    /**
     * Better Combat's attack, as its client would request it: the combo (counted up while
     * attacks follow within its reset window), the targets its TargetFinder finds among the
     * copies, then its server handler. False when the held item has no Better Combat attack
     * (vanilla's attack then).
     */
    private static boolean betterCombatAttack(ServerPlayer player, ServerLevel level, Entity cursor) {
        try {
            ClassLoader loader = HorizonCombat.class.getClassLoader();
            Class<?> registry = Class.forName("net.bettercombat.logic.WeaponRegistry", true, loader);
            Object attributes = registry.getMethod("getAttributes", net.minecraft.world.item.ItemStack.class).invoke(null, player.getMainHandItem());
            if (attributes == null) return false;
            long now = HorizonServer.currentTick();
            long[] combo = COMBO.computeIfAbsent(player.getUUID(), u -> new long[] {0, Long.MIN_VALUE / 2});
            // Better Combat's client resets the combo after a pause (ticks past the cooldown).
            if (now - combo[1] > 40) combo[0] = 0;
            int count = (int) combo[0];
            combo[0]++;
            combo[1] = now;
            Class<?> helper = Class.forName("net.bettercombat.logic.PlayerAttackHelper", true, loader);
            Object hand = helper.getMethod("getCurrentAttack", Player.class, int.class).invoke(null, player, count);
            if (hand == null) return false;
            Object attack = hand.getClass().getMethod("attack").invoke(hand);
            Object handStack = hand.getClass().getMethod("itemStack").invoke(hand);
            double range = (double) helper.getMethod("getRangeForItem", Player.class, net.minecraft.world.item.ItemStack.class)
                    .invoke(null, player, handStack);
            Class<?> finder = Class.forName("net.bettercombat.client.collision.TargetFinder", true, loader);
            Class<?> attackClass = Class.forName("net.bettercombat.api.WeaponAttributes$Attack", true, loader);
            @SuppressWarnings("unchecked")
            List<Entity> targets = (List<Entity>) finder.getMethod("findAttackTargets", Player.class, Entity.class, attackClass, double.class)
                    .invoke(null, player, cursor, attack, range);
            if (DEBUG) Natives.log("combat: Better Combat's targets " + targets);
            if (targets == null || targets.isEmpty()) targets = List.of(cursor);
            Class<?> request = Class.forName("net.bettercombat.network.Packets$C2S_AttackRequest", true, loader);
            Object packet = request.getConstructor(int.class, boolean.class, int.class, Entity.class, List.class)
                    .newInstance(count, player.isShiftKeyDown(), player.getInventory().getSelectedSlot(), cursor, targets);
            Class<?> network = Class.forName("net.bettercombat.network.ServerNetwork", true, loader);
            var handle = network.getMethod("handleAttackRequest", request, net.minecraft.server.MinecraftServer.class, ServerPlayer.class,
                    net.minecraft.server.network.ServerGamePacketListenerImpl.class);
            List<Entity> chosen = targets;
            HorizonServer.fire("Better Combat's attack", () -> {
                try {
                    handle.invoke(null, packet, HorizonServer.server(), player, player.connection);
                } catch (ReflectiveOperationException e) {
                    throw new IllegalStateException(e.getCause() != null ? e.getCause() : e);
                }
            });
            if (DEBUG) Natives.log("combat: Better Combat combo " + count + ", " + chosen.size() + " target(s) in " + range + " blocks");
            return true;
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("better combat attack")) Natives.log("Java mods: Better Combat's attack couldn't run: " + t);
            return false;
        }
    }
}
