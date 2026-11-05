package dev.horizonlab.platform;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.InsideBlockEffectApplier;
import net.minecraft.world.entity.MoverType;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.Shapes;

/**
 * Horizon's own item entities as the mods' blocks and machines see them. Horizon runs items
 * (their physics, merging, pickup); the mirror never had them, so an item thrown onto a mod's
 * block did nothing: a depot or a belt it lands on, crushing wheels it falls between, a
 * millstone, a fan blowing on it. Each tick Horizon lists its items near its players
 * (`horizonItems`), and each stands in the mirrored level as an ItemEntity of its own (not
 * ticked) with Horizon's id, place, motion and stack, kept from tick to tick (a machine holds
 * on to the item it works on, and mods keep data on it). The mods' blocks get what vanilla's
 * Entity.move gives them for it: the move's patches with the block it rests on (Entity.move,
 * with no movement: Horizon moved it already), and entityInside for the mods' blocks it is
 * in. Mods' block entities find them as any entity (HorizonServer.entities: a fan's air
 * current). After the tick what the mods did goes back (`report`): an item taken in (removed),
 * changed (a fan's smelting), pushed or moved, which Horizon applies to its own. An item a
 * mod's block holds (HELD) moves here instead, by vanilla's tick, while it touches the block.
 */
public final class HorizonItems {
    private HorizonItems() {
    }

    /** Per level, the shadows by Horizon's item id. */
    private static final Map<ServerLevel, Map<Integer, ItemEntity>> SHADOWS = new IdentityHashMap<>();

    /** What Horizon last showed of a shadow: the stack (its SNBT as sent, and the stack), place, motion. */
    private record Seen(String snbt, ItemStack stack, Vec3 pos, Vec3 vel) {
    }

    private static final Map<ItemEntity, Seen> SEEN = new IdentityHashMap<>();
    /**
     * Items a mod's block holds: touching one, and moved or changed by the mods. Their moves are
     * the mirror's (Horizon skips its own tick for them): a block's collision may depend on the
     * entity (crushing wheels let the item they work on fall through), which Horizon's one shape
     * per state can't answer. Vanilla's whole ItemEntity.tick runs for them here.
     */
    private static final java.util.Set<ItemEntity> HELD = java.util.Collections.newSetFromMap(new IdentityHashMap<>());
    /** The items that touched a mod's block this tick. */
    private static final java.util.Set<ItemEntity> TOUCHED = java.util.Collections.newSetFromMap(new IdentityHashMap<>());
    /** Whether the last report held any item (an empty list is sent once, to let them go). */
    private static boolean heldSent;
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("items");

    /**
     * Horizon's items this tick, `{"<dimension>": [[id, x, y, z, vx, vy, vz, on_ground, age,
     * pickup_delay, "stack snbt"], ...]}`: the shadows follow, new ones are made, those no
     * longer listed (picked up, merged, gone, far) go.
     */
    public static synchronized void update(String json) {
        Map<ServerLevel, Map<Integer, ItemEntity>> seen = new IdentityHashMap<>();
        var root = JsonParser.parseString(json).getAsJsonObject();
        for (var dim : root.entrySet()) {
            ServerLevel level = HorizonServer.level(dim.getKey());
            if (level == null) continue;
            Map<Integer, ItemEntity> had = SHADOWS.getOrDefault(level, Map.of());
            Map<Integer, ItemEntity> now = new HashMap<>();
            for (JsonElement el : dim.getValue().getAsJsonArray()) {
                JsonArray a = el.getAsJsonArray();
                int id = a.get(0).getAsInt();
                ItemEntity shadow = had.get(id);
                Vec3 at = new Vec3(a.get(1).getAsDouble(), a.get(2).getAsDouble(), a.get(3).getAsDouble());
                if (shadow == null || shadow.isRemoved()) shadow = adopt(level, id, at, a.get(10).getAsString());
                if (shadow == null) shadow = make(level, id);
                if (shadow == null) continue;
                Vec3 pos = at;
                Vec3 vel = new Vec3(a.get(4).getAsDouble(), a.get(5).getAsDouble(), a.get(6).getAsDouble());
                shadow.setOldPosAndRot();
                shadow.setPos(pos);
                shadow.setDeltaMovement(vel);
                shadow.setOnGround(a.get(7).getAsInt() != 0);
                setAge(shadow, a.get(8).getAsInt());
                shadow.setPickUpDelay(a.get(9).getAsInt());
                String snbt = a.get(10).getAsString();
                Seen before = SEEN.get(shadow);
                if (before == null || !before.snbt().equals(snbt)) shadow.setItem(HorizonServer.item(snbt));
                SEEN.put(shadow, new Seen(snbt, shadow.getItem().copy(), pos, vel));
                now.put(id, shadow);
            }
            seen.put(level, now);
        }
        for (var level : SHADOWS.entrySet()) {
            Map<Integer, ItemEntity> now = seen.getOrDefault(level.getKey(), Map.of());
            for (var shadow : level.getValue().entrySet()) {
                if (now.get(shadow.getKey()) != shadow.getValue()) {
                    shadow.getValue().setRemoved(Entity.RemovalReason.DISCARDED);
                    SEEN.remove(shadow.getValue());
                    HELD.remove(shadow.getValue());
                }
            }
        }
        SHADOWS.clear();
        SHADOWS.putAll(seen);
    }

    /** An item a mod's code spawned (HorizonServer.addEntity), waiting for Horizon's item it became. */
    private record Spawned(ServerLevel level, ItemEntity item, long tick) {
    }

    private static final List<Spawned> SPAWNED = new ArrayList<>();

    /**
     * A mod's code added an item to a mirrored level (crushing wheels' output, a machine's
     * drop): Horizon makes it its own item (Natives.dropItem). The mod's object is kept to stand
     * for it once Horizon lists it (`adopt`), with what the mod set on it (Create marks its
     * output so the wheels don't pull it in again).
     */
    public static synchronized void spawned(ServerLevel level, ItemEntity item) {
        long now = level.getGameTime();
        SPAWNED.removeIf(s -> now - s.tick() > 40);
        if (SPAWNED.size() < 256) SPAWNED.add(new Spawned(level, item, now));
    }

    /** The spawned item Horizon's new item {@code id} is (same level and stack, near), as its shadow. */
    private static ItemEntity adopt(ServerLevel level, int id, Vec3 at, String snbt) {
        if (SPAWNED.isEmpty()) return null;
        ItemStack stack = HorizonServer.item(snbt);
        for (var it = SPAWNED.iterator(); it.hasNext(); ) {
            Spawned s = it.next();
            ItemStack had = s.item().getItem();
            if (s.level() != level || !ItemStack.isSameItem(had, stack) || had.getCount() != stack.getCount()
                    || s.item().position().distanceToSqr(at) > 1.0) continue;
            it.remove();
            s.item().setId(id);
            if (DEBUG) Natives.log("items: " + had + " a mod spawned stands for Horizon's item " + id);
            return s.item();
        }
        return null;
    }

    private static ItemEntity make(ServerLevel level, int id) {
        try {
            ItemEntity item = net.minecraft.world.entity.EntityTypes.ITEM.create(level, EntitySpawnReason.LOAD);
            if (item == null) return null;
            item.setId(id);
            return item;
        } catch (Throwable t) {
            Natives.log("Java mods: Horizon's items can't stand in the mirror: " + t);
            return null;
        }
    }

    private static java.lang.reflect.Field age;

    /** ItemEntity.age (private; its getter is the client's): Horizon's, for mods that read it. */
    private static void setAge(ItemEntity item, int value) {
        try {
            if (age == null) {
                age = ItemEntity.class.getDeclaredField("age");
                age.setAccessible(true);
            }
            age.setInt(item, value);
        } catch (ReflectiveOperationException ignored) {
        }
    }

    /** The shadows in a level, alive (HorizonServer.entities lists them with the mods' entities). */
    public static synchronized List<Entity> of(ServerLevel level) {
        Map<Integer, ItemEntity> s = SHADOWS.get(level);
        if (s == null || s.isEmpty()) return List.of();
        List<Entity> out = new ArrayList<>(s.size());
        for (ItemEntity e : s.values()) if (!e.isRemoved()) out.add(e);
        return out;
    }

    /** Whether {@code entity} stands in for one of Horizon's items. */
    public static synchronized boolean isShadow(Object entity) {
        return entity instanceof ItemEntity e && SEEN.containsKey(e);
    }

    // ---- What vanilla's move gives the blocks an item touches ------------------------------

    /** Per block class: whether it (a mod's class) answers entityInside itself. */
    private static final Map<Class<?>, Boolean> INSIDE = new ConcurrentHashMap<>();

    private static boolean ownsEntityInside(Block block) {
        return INSIDE.computeIfAbsent(block.getClass(), c -> {
            for (Class<?> k = c; k != null && k != Block.class; k = k.getSuperclass()) {
                if (k.getName().startsWith("net.minecraft.")) continue;
                for (var m : k.getDeclaredMethods()) {
                    if (m.getName().equals("entityInside") && m.getParameterCount() == 6) return true;
                }
            }
            return false;
        });
    }

    private static java.lang.reflect.Method speedFactor;

    /** Entity.getBlockSpeedFactor (protected): what Entity.move scales the motion by at its end. */
    private static float speedFactor(Entity entity) {
        try {
            if (speedFactor == null) {
                speedFactor = Entity.class.getDeclaredMethod("getBlockSpeedFactor");
                speedFactor.setAccessible(true);
            }
            return (float) speedFactor.invoke(entity);
        } catch (ReflectiveOperationException e) {
            return 1.0F;
        }
    }

    /**
     * Before the mods' block entities tick: each item on or in a mod's block gets what that
     * block gets from vanilla's movement. On one: Entity.move, with no movement (Horizon moved
     * the item: the mods' patches of move run with the block under it, as a depot's or a belt's
     * catching what lands on it); its own effects on the item undone (the ground, the block's
     * speed factor: Horizon's). In one: the block's entityInside, as checkInsideBlocks calls it
     * where the item's box meets the block's inside shape (crushing wheels pulling it in).
     */
    public static void touchBlocks() {
        List<Map.Entry<ServerLevel, ItemEntity>> all = new ArrayList<>();
        synchronized (HorizonItems.class) {
            for (var level : SHADOWS.entrySet()) {
                for (ItemEntity e : level.getValue().values()) all.add(Map.entry(level.getKey(), e));
            }
        }
        synchronized (HorizonItems.class) {
            TOUCHED.clear();
        }
        for (var entry : all) {
            ServerLevel level = entry.getKey();
            ItemEntity item = entry.getValue();
            if (item.isRemoved()) continue;
            boolean held;
            synchronized (HorizonItems.class) {
                held = HELD.contains(item);
            }
            if (held) {
                // A held item: vanilla's tick here (its move with the mods' shapes and patches,
                // the blocks it is in), Horizon having left it.
                HorizonServer.fire("ItemEntity.tick", item::tick);
                if (DEBUG) Natives.log("items: " + item.getItem() + " held, ticked here: " + item.position() + ", " + item.getDeltaMovement());
                if (!item.isRemoved() && touchesModBlock(level, item)) {
                    synchronized (HorizonItems.class) {
                        TOUCHED.add(item);
                    }
                }
                continue;
            }
            if (touchesModBlock(level, item)) {
                synchronized (HorizonItems.class) {
                    TOUCHED.add(item);
                }
            }
            BlockPos on = item.getOnPosLegacy();
            if (HorizonBlockEntities.modded(level.getBlockState(on))) {
                Vec3 vel = item.getDeltaMovement();
                Vec3 pos = item.position();
                boolean ground = item.onGround();
                float factor = speedFactor(item);
                HorizonServer.fire("Entity.move", () -> item.move(MoverType.SELF, Vec3.ZERO));
                if (!item.isRemoved()) {
                    Vec3 after = item.getDeltaMovement();
                    if (after.equals(new Vec3(vel.x * factor, vel.y, vel.z * factor))) item.setDeltaMovement(vel);
                    item.setOnGround(ground);
                    if (DEBUG && (!after.equals(vel) || !item.position().equals(pos))) {
                        Natives.log("items: " + item.getItem() + " moved by the block under it to " + item.position() + ", " + item.getDeltaMovement());
                    }
                }
            }
            if (item.isRemoved()) {
                if (DEBUG) Natives.log("items: " + item.getItem() + " taken by " + level.getBlockState(on) + " under it");
                continue;
            }
            AABB box = item.getBoundingBox().deflate(1.0E-5F);
            for (BlockPos pos : BlockPos.betweenClosed(BlockPos.containing(box.minX, box.minY, box.minZ),
                    BlockPos.containing(box.maxX, box.maxY, box.maxZ))) {
                BlockState state = level.getBlockState(pos);
                if (!HorizonBlockEntities.modded(state) || !ownsEntityInside(state.getBlock())) continue;
                var shape = state.getEntityInsideCollisionShape(level, pos, item);
                boolean inside = shape == Shapes.block();
                if (!inside) {
                    for (AABB part : shape.move(new Vec3(pos.getX(), pos.getY(), pos.getZ())).toAabbs()) {
                        if (part.intersects(box)) {
                            inside = true;
                            break;
                        }
                    }
                }
                if (!inside) continue;
                BlockPos at = pos.immutable();
                HorizonServer.fire("BlockBehaviour.entityInside", () ->
                        state.entityInside(level, at, item, InsideBlockEffectApplier.NOOP, true));
                if (DEBUG) Natives.log("items: " + item.getItem() + " inside " + state + (item.isRemoved() ? ": taken" : ""));
                if (item.isRemoved()) break;
            }
        }
    }

    /** Whether the item's box, or the block it rests on, meets a mod's block. */
    private static boolean touchesModBlock(ServerLevel level, ItemEntity item) {
        AABB box = item.getBoundingBox().expandTowards(0, -0.1, 0);
        for (BlockPos pos : BlockPos.betweenClosed(BlockPos.containing(box.minX, box.minY, box.minZ),
                BlockPos.containing(box.maxX, box.maxY, box.maxZ))) {
            if (HorizonBlockEntities.modded(level.getBlockState(pos))) return true;
        }
        return false;
    }

    /**
     * After the tick: what the mods did to the shadows, to Horizon (Natives.itemsChanged): an
     * item removed (taken in, emptied), its stack changed, moved or pushed. Each change once:
     * what is sent is what Horizon will show next tick.
     */
    public static void report() {
        JsonArray out = new JsonArray();
        JsonArray held = new JsonArray();
        synchronized (HorizonItems.class) {
            for (var level : SHADOWS.entrySet()) {
                String dimension = level.getKey().dimension().identifier().toString();
                for (var it = level.getValue().entrySet().iterator(); it.hasNext(); ) {
                    var entry = it.next();
                    ItemEntity item = entry.getValue();
                    Seen seen = SEEN.get(item);
                    if (seen == null) continue;
                    JsonObject change = new JsonObject();
                    if (item.isRemoved() || item.getItem().isEmpty()) {
                        change.addProperty("removed", true);
                        it.remove();
                        SEEN.remove(item);
                        HELD.remove(item);
                    } else {
                        ItemStack stack = item.getItem();
                        String snbt = seen.snbt();
                        if (!ItemStack.matches(seen.stack(), stack)) {
                            snbt = HorizonServer.snbt(stack);
                            change.addProperty("stack", snbt);
                        }
                        if (!item.position().equals(seen.pos())) change.add("pos", vec(item.position()));
                        if (!item.getDeltaMovement().equals(seen.vel())) change.add("vel", vec(item.getDeltaMovement()));
                        SEEN.put(item, new Seen(snbt == null ? "" : snbt, stack.copy(), item.position(), item.getDeltaMovement()));
                        // Held while it touches a mod's block, from when the mods first moved or
                        // changed it there (one only resting on a mod's block stays Horizon's).
                        if (TOUCHED.contains(item) && (HELD.contains(item) || change.size() > 0)) {
                            HELD.add(item);
                            held.add(entry.getKey());
                        } else {
                            HELD.remove(item);
                        }
                    }
                    if (change.size() == 0) continue;
                    change.addProperty("d", dimension);
                    change.addProperty("id", entry.getKey());
                    out.add(change);
                }
            }
        }
        if (out.isEmpty() && held.isEmpty() && !heldSent) return;
        heldSent = !held.isEmpty();
        JsonObject report = new JsonObject();
        report.add("changes", out);
        report.add("held", held);
        if (DEBUG && (!out.isEmpty() || !held.isEmpty())) Natives.log("items: " + report);
        Natives.itemsChanged(report.toString());
    }

    private static JsonArray vec(Vec3 v) {
        JsonArray a = new JsonArray();
        a.add(v.x);
        a.add(v.y);
        a.add(v.z);
        return a;
    }
}
