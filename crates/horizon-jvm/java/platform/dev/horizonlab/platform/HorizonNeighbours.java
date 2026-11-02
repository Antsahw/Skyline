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
}
