package dev.horizonlab.platform;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Mob;

/**
 * Horizon's own mobs as the mods' mobs see them: each tick Horizon lists its mobs near its
 * players (`horizonMobs`), and each stands in the mirrored level as a shadow of its type (no AI,
 * not ticked) at its place, with its health. Mods' mobs find them as they find any entity
 * (HorizonServer.entities: a wolf of a mod hunts Horizon's sheep, a mod's rabbit flees
 * Horizon's fox), and what they do to one goes to Horizon: a hurt is Horizon's mob's
 * (CombatCaptureMixin → `hurt` → Natives.hurtMob), which Horizon applies next tick.
 */
public final class HorizonNeighbours {
    private HorizonNeighbours() {
    }

    /** Per level, the shadows by Horizon's mob id. */
    private static final Map<ServerLevel, Map<Integer, LivingEntity>> SHADOWS = new IdentityHashMap<>();
    /** Every shadow, for the quick "is this one?" of the hurt mixin. */
    private static final Map<Entity, ServerLevel> ALL = new IdentityHashMap<>();
    private static final Map<String, Boolean> REFUSED = new HashMap<>();

    /**
     * Horizon's mobs this tick, `{"<dimension>": [[id, "type", x, y, z, yaw, health], ...]}`:
     * the shadows follow, new ones are made, those no longer listed (dead, gone, far) go.
     */
    public static synchronized void update(String json) {
        Map<ServerLevel, Map<Integer, LivingEntity>> seen = new IdentityHashMap<>();
        var root = JsonParser.parseString(json).getAsJsonObject();
        for (var dim : root.entrySet()) {
            ServerLevel level = HorizonServer.level(dim.getKey());
            if (level == null) continue;
            Map<Integer, LivingEntity> had = SHADOWS.getOrDefault(level, Map.of());
            Map<Integer, LivingEntity> now = new HashMap<>();
            for (JsonElement el : dim.getValue().getAsJsonArray()) {
                JsonArray a = el.getAsJsonArray();
                int id = a.get(0).getAsInt();
                LivingEntity shadow = had.get(id);
                if (shadow == null) shadow = make(level, id, a.get(1).getAsString());
                if (shadow == null) continue;
                double x = a.get(2).getAsDouble(), y = a.get(3).getAsDouble(), z = a.get(4).getAsDouble();
                float yaw = a.get(5).getAsFloat();
                shadow.setOldPosAndRot();
                shadow.setPos(x, y, z);
                shadow.setYRot(yaw);
                shadow.setYHeadRot(yaw);
                shadow.setYBodyRot(yaw);
                shadow.setHealth(Math.min(a.get(6).getAsFloat(), shadow.getMaxHealth()));
                now.put(id, shadow);
            }
            seen.put(level, now);
        }
        // Gone from Horizon's list: removed for the mods too (their targets let go).
        for (var level : SHADOWS.entrySet()) {
            Map<Integer, LivingEntity> now = seen.getOrDefault(level.getKey(), Map.of());
            for (var shadow : level.getValue().entrySet()) {
                if (!now.containsKey(shadow.getKey())) {
                    shadow.getValue().setRemoved(Entity.RemovalReason.DISCARDED);
                    ALL.remove(shadow.getValue());
                }
            }
        }
        SHADOWS.clear();
        SHADOWS.putAll(seen);
    }

    private static LivingEntity make(ServerLevel level, int id, String typeName) {
        try {
            var key = net.minecraft.resources.Identifier.tryParse(typeName);
            var type = key == null ? null : net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getOptional(key).orElse(null);
            if (type == null || REFUSED.containsKey(typeName)) return null;
            Entity e = type.create(level, EntitySpawnReason.LOAD);
            if (!(e instanceof LivingEntity living)) {
                REFUSED.put(typeName, true);
                return null;
            }
            living.setId(id);
            if (living instanceof Mob mob) mob.setNoAi(true);
            ALL.put(living, level);
            return living;
        } catch (Throwable t) {
            if (REFUSED.put(typeName, true) == null) Natives.log("Java mods: Horizon's " + typeName + " can't stand in the mirror: " + t);
            return null;
        }
    }

    /** The shadows in a level (HorizonServer.entities lists them with the mods' entities). */
    public static synchronized List<Entity> of(ServerLevel level) {
        // A routed attack brings its own copies of these mobs (HorizonCombat): not twice.
        if (!HorizonCombat.visiting(level).isEmpty()) return List.of();
        Map<Integer, LivingEntity> s = SHADOWS.get(level);
        return s == null ? List.of() : new ArrayList<>(s.values());
    }

    /** Whether {@code entity} stands in for one of Horizon's mobs. */
    public static boolean isShadow(Object entity) {
        synchronized (HorizonNeighbours.class) {
            return entity instanceof Entity e && ALL.containsKey(e);
        }
    }

    /**
     * LivingEntity.hurtServer on a shadow (CombatCaptureMixin): Horizon hurts its mob, as its
     * own damage of this type from the attacker's place (its armour, its invulnerable ticks).
     */
    public static void hurt(LivingEntity shadow, DamageSource source, float amount) {
        ServerLevel level;
        synchronized (HorizonNeighbours.class) {
            level = ALL.get(shadow);
        }
        if (level == null || amount <= 0.0F) return;
        String type = source.typeHolder().unwrapKey().map(k -> k.identifier().toString()).orElse("minecraft:generic");
        var from = source.getSourcePosition();
        Entity by = source.getEntity() != null ? source.getEntity() : source.getDirectEntity();
        Natives.hurtMob(level.dimension().identifier().toString(), shadow.getId(), type, amount,
                from == null ? Double.NaN : from.x, from == null ? Double.NaN : from.y, from == null ? Double.NaN : from.z,
                by == null ? -1 : by.getId());
    }

    /**
     * The vanilla type Horizon's mobs take a mod's mob for (they know only vanilla's): its nearest
     * vanilla class among those their goals look for (a zombie, a skeleton, a wolf, a villager...),
     * else a spider for a monster (an Enemy iron golems attack, that villagers don't flee), a
     * cod for a fish or squid, a pig for the rest.
     */
    static String like(Entity e) {
        return LIKE.computeIfAbsent(e.getClass(), c -> {
            for (var pair : LIKES) if (pair.getKey().isAssignableFrom(c)) return pair.getValue();
            return "minecraft:pig";
        });
    }

    private static final Map<Class<?>, String> LIKE = new java.util.concurrent.ConcurrentHashMap<>();
    /** Most specific first. */
    private static final List<Map.Entry<Class<?>, String>> LIKES = List.of(
            Map.entry(net.minecraft.world.entity.monster.zombie.Zombie.class, "minecraft:zombie"),
            Map.entry(net.minecraft.world.entity.monster.skeleton.AbstractSkeleton.class, "minecraft:skeleton"),
            Map.entry(net.minecraft.world.entity.monster.Creeper.class, "minecraft:creeper"),
            Map.entry(net.minecraft.world.entity.monster.spider.Spider.class, "minecraft:spider"),
            Map.entry(net.minecraft.world.entity.npc.villager.AbstractVillager.class, "minecraft:villager"),
            Map.entry(net.minecraft.world.entity.monster.illager.AbstractIllager.class, "minecraft:pillager"),
            Map.entry(net.minecraft.world.entity.animal.wolf.Wolf.class, "minecraft:wolf"),
            Map.entry(net.minecraft.world.entity.animal.sheep.Sheep.class, "minecraft:sheep"),
            Map.entry(net.minecraft.world.entity.animal.chicken.Chicken.class, "minecraft:chicken"),
            Map.entry(net.minecraft.world.entity.animal.rabbit.Rabbit.class, "minecraft:rabbit"),
            Map.entry(net.minecraft.world.entity.animal.fish.AbstractFish.class, "minecraft:cod"),
            Map.entry(net.minecraft.world.entity.animal.squid.Squid.class, "minecraft:cod"),
            Map.entry(net.minecraft.world.entity.monster.Enemy.class, "minecraft:spider"));

    /**
     * The mods' living entities as Horizon's mobs see them, per dimension:
     * `"<dimension>": [[id, "like", x, y, z, width, height, health, yaw, hostile], ...],` without
     * the outer braces (HorizonEntityRender's snapshot writes them).
     */
    static void seen(StringBuilder out) {
        boolean firstDim = true;
        for (var level : HorizonEntities.all().entrySet()) {
            boolean first = true;
            for (Entity e : level.getValue()) {
                if (!(e instanceof LivingEntity l) || e.isRemoved() || !l.isAlive()) continue;
                if (first) {
                    if (!firstDim) out.append(',');
                    firstDim = false;
                    HorizonJson.string(level.getKey().dimension().identifier().toString(), out);
                    out.append(":[");
                } else {
                    out.append(',');
                }
                first = false;
                out.append('[').append(e.getId()).append(',');
                HorizonJson.string(like(e), out);
                out.append(',').append(e.getX()).append(',').append(e.getY()).append(',').append(e.getZ())
                        .append(',').append(e.getBbWidth()).append(',').append(e.getBbHeight())
                        .append(',').append(l.getHealth()).append(',').append(e.getYRot())
                        .append(',').append(e instanceof net.minecraft.world.entity.monster.Enemy ? 1 : 0).append(']');
            }
            if (!first) out.append(']');
        }
    }

    /**
     * Horizon's mob `attacker` hit the mods' entity `id` for `amount` (Horizon's own melee: a
     * golem's swing, a wolf's bite). A mob attack by the attacker's shadow, so the entity strikes
     * back at it (HurtByTargetGoal). False when there is no such entity or it took nothing.
     */
    public static boolean hurtByHorizon(int id, int attacker, float amount) {
        Entity target = HorizonEntities.byId(id);
        if (!(target instanceof LivingEntity living) || !(target.level() instanceof ServerLevel level)) return false;
        LivingEntity shadow;
        synchronized (HorizonNeighbours.class) {
            shadow = SHADOWS.getOrDefault(level, Map.of()).get(attacker);
        }
        DamageSource source = shadow != null ? level.damageSources().mobAttack(shadow) : level.damageSources().generic();
        boolean[] hurt = {false};
        HorizonEntities.fire(target, "LivingEntity.hurtServer (Horizon's mob)", () -> hurt[0] = living.hurtServer(level, source, amount));
        // hurtServer's own knockback pushes it away from the shadow.
        return hurt[0];
    }
}
