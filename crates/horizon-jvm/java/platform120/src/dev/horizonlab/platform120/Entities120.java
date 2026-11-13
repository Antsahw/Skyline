package dev.horizonlab.platform120;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.AbortableIterationConsumer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.entity.EntityTypeTest;
import net.minecraft.world.level.entity.LevelEntityGetter;
import net.minecraft.world.phys.AABB;

/**
 * The 1.20.1 mods' own entities (phase 6d): a Friends & Foes moobloom, a copper golem. They
 * live here, in the mirror level they were made in, and tick here with their mod's code: their
 * goals, their paths and their physics against Horizon's blocks (the mirror's reads). Vanilla's
 * entities are Horizon's, and never come here.
 *
 * <p>Each level's are its {@link LevelEntityGetter}, which is what vanilla asks for the entities
 * around (a mob looking for food, a golem for a target), so they see each other. Ids come from a
 * range of their own, away from Horizon's ({@link #FIRST_ID}), so Horizon can tell whose an
 * entity it is told about is.
 */
public final class Entities120 implements LevelEntityGetter<Entity> {
    /** Where the mods' entities' ids start: Horizon's own are below. */
    public static final int FIRST_ID = 0x7ffe_0000;

    private static final Map<Level, Entities120> LEVELS = new HashMap<>();
    private static int nextId = FIRST_ID;

    private final List<Entity> entities = new ArrayList<>();

    private Entities120() {}

    /** A level's entities, made empty the first time it is asked for. */
    public static synchronized Entities120 of(Level level) {
        return LEVELS.computeIfAbsent(level, l -> new Entities120());
    }

    /** Whether `entity` is a mod's: not vanilla's (Horizon's) and not a player. */
    public static boolean modded(Entity entity) {
        if (entity instanceof net.minecraft.world.entity.player.Player) return false;
        return !"minecraft".equals(BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).getNamespace());
    }

    /** Level.addFreshEntity for a mod's entity: it lives here from now on. */
    public static synchronized boolean add(Level level, Entity entity) {
        Entities120 here = of(level);
        if (here.entities.contains(entity)) return false;
        entity.setId(nextId++);
        here.entities.add(entity);
        return true;
    }

    /** The entity with this id, in any level. */
    public static synchronized Entity byId(int id) {
        for (Entities120 here : LEVELS.values()) {
            Entity e = here.get(id);
            if (e != null) return e;
        }
        return null;
    }

    private static final Set<String> FAILED = new HashSet<>();

    /**
     * A tick of a level's entities, as ServerLevel.tick ticks them: each alive one ticks (its
     * mod's code), and those removed since (killed, despawned, picked up) are gone after.
     */
    public static void tick(Level level) {
        List<Entity> now;
        synchronized (Entities120.class) {
            now = new ArrayList<>(of(level).entities);
        }
        // Those in chunks Horizon hasn't loaded wait, as vanilla's do in chunks that don't
        // tick: else they'd fall through the air an unloaded chunk reads as.
        long[] chunks = now.stream().mapToLong(e -> net.minecraft.world.level.ChunkPos.asLong(e.blockPosition())).distinct().toArray();
        Set<Long> loaded = Mirror120.loadedChunks(level, chunks);
        for (Entity entity : now) {
            if (entity.isRemoved()) continue;
            if (!loaded.contains(net.minecraft.world.level.ChunkPos.asLong(entity.blockPosition()))) continue;
            try {
                entity.setOldPosAndRot();
                entity.tickCount++;
                entity.tick();
            } catch (RuntimeException | LinkageError e) {
                String type = BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
                if (FAILED.add(type)) {
                    Mirror120.log("a " + type + " failed ticking: " + e);
                    for (StackTraceElement at : e.getStackTrace()) Mirror120.log("    at " + at);
                }
            }
        }
        synchronized (Entities120.class) {
            of(level).entities.removeIf(Entity::isRemoved);
        }
    }

    /**
     * What Horizon needs of each of a level's entities, eleven numbers each: id, x, y, z, yaw,
     * pitch, head yaw, body yaw, width, height, health (-1 for one that has none); and its
     * type, beside.
     */
    public static synchronized Object[] snapshot(Level level) {
        List<Entity> all = of(level).entities;
        double[] numbers = new double[all.size() * 11];
        String[] types = new String[all.size()];
        for (int i = 0; i < all.size(); i++) {
            Entity e = all.get(i);
            int at = i * 11;
            numbers[at] = e.getId();
            numbers[at + 1] = e.getX();
            numbers[at + 2] = e.getY();
            numbers[at + 3] = e.getZ();
            numbers[at + 4] = e.getYRot();
            numbers[at + 5] = e.getXRot();
            numbers[at + 6] = e.getYHeadRot();
            numbers[at + 7] = e instanceof LivingEntity l ? l.yBodyRot : e.getYRot();
            numbers[at + 8] = e.getBbWidth();
            numbers[at + 9] = e.getBbHeight();
            numbers[at + 10] = e instanceof LivingEntity l ? l.getHealth() : -1;
            types[i] = BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString();
        }
        return new Object[] {numbers, types};
    }

    /**
     * A level's entities as 1.20.1 saves them (Entity.save: with their type's id), in SNBT, to
     * be kept with Horizon's world; passengers inside their vehicle, as vanilla keeps them.
     */
    public static synchronized String[] save(Level level) {
        List<String> out = new ArrayList<>();
        for (Entity e : of(level).entities) {
            if (e.isRemoved() || e.isPassenger()) continue;
            var tag = new net.minecraft.nbt.CompoundTag();
            try {
                if (e.save(tag)) out.add(tag.toString());
            } catch (RuntimeException | LinkageError x) {
                Mirror120.log("a " + BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()) + " failed saving: " + x);
            }
        }
        return out.toArray(new String[0]);
    }

    /**
     * Entities a save kept, back into a level (EntityType.loadEntityRecursive, as a chunk's
     * entities load); one whose UUID is already here is left out. How many came back.
     */
    public static int load(Level level, String[] saved) {
        int loaded = 0;
        for (String snbt : saved) {
            try {
                var tag = net.minecraft.nbt.TagParser.parseTag(snbt);
                Entity top = net.minecraft.world.entity.EntityType.loadEntityRecursive(tag, level, e -> {
                    synchronized (Entities120.class) {
                        if (of(level).get(e.getUUID()) != null) return null;
                    }
                    return e;
                });
                if (top == null) continue;
                for (Entity e : top.getSelfAndPassengers().toList()) {
                    if (add(level, e)) loaded++;
                }
            } catch (Exception x) {
                Mirror120.log("a saved entity didn't load: " + x);
            }
        }
        return loaded;
    }

    /** Every level's entities gone: the world they lived in closed. */
    public static synchronized void clear() {
        LEVELS.clear();
    }

    @Override
    public synchronized Entity get(int id) {
        for (Entity e : entities) if (e.getId() == id) return e;
        return null;
    }

    @Override
    public synchronized Entity get(UUID uuid) {
        for (Entity e : entities) if (e.getUUID().equals(uuid)) return e;
        return null;
    }

    @Override
    public synchronized Iterable<Entity> getAll() {
        return new ArrayList<>(entities);
    }

    @Override
    public <U extends Entity> void get(EntityTypeTest<Entity, U> test, AbortableIterationConsumer<U> consumer) {
        for (Entity e : getAll()) {
            U u = test.tryCast(e);
            if (u != null && consumer.accept(u).shouldAbort()) return;
        }
    }

    @Override
    public void get(AABB box, Consumer<Entity> consumer) {
        for (Entity e : getAll()) if (e.getBoundingBox().intersects(box)) consumer.accept(e);
    }

    @Override
    public <U extends Entity> void get(EntityTypeTest<Entity, U> test, AABB box, AbortableIterationConsumer<U> consumer) {
        for (Entity e : getAll()) {
            if (!e.getBoundingBox().intersects(box)) continue;
            U u = test.tryCast(e);
            if (u != null && consumer.accept(u).shouldAbort()) return;
        }
    }
}
