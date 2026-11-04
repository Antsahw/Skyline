package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.List;

import net.fabricmc.fabric.api.transfer.v1.item.ContainerStorage;
import net.fabricmc.fabric.api.transfer.v1.item.ItemStorage;
import net.fabricmc.fabric.api.transfer.v1.item.ItemVariant;
import net.fabricmc.fabric.api.transfer.v1.storage.Storage;
import net.fabricmc.fabric.api.transfer.v1.storage.StorageView;
import net.fabricmc.fabric.api.transfer.v1.transaction.Transaction;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.Mth;
import net.minecraft.world.Container;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.AABB;

/**
 * Horizon's hoppers (Rust) moving items into and out of a Java mod's block: Farmer's Delight's
 * cooking pot, Energized Power's machines, mods' chests. Horizon's world has no block entity
 * there (the mod's lives in Java: HorizonBlockEntities.LIVE), so its hopper asks here, as
 * Fabric Transfer API's patch of vanilla's ejectItems/suckInItems asks: the block's
 * ItemStorage.SIDED on the face the hopper touches (which also wraps a plain vanilla Container
 * or WorldlyContainer a mod's block entity is). One item a call, as a hopper moves. Else a mod's
 * container entity in that block (a mod's minecart), as vanilla's getContainerAt finds one.
 * Horizon's droppers ask too (Fabric Transfer API's DropperBlockMixin: a dropper with no
 * vanilla container in front moves one item into the block's storage).
 */
final class HorizonHoppers {
    private HorizonHoppers() {}

    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("hoppers");

    private static Storage<ItemVariant> storage(ServerLevel level, BlockPos pos, int face) {
        Direction side = Direction.from3DDataValue(face);
        Storage<ItemVariant> block = ItemStorage.SIDED.find(level, pos, side);
        if (block != null) return block;
        // No block storage: a mod's container entity there (a mod's chest minecart), as
        // vanilla's HopperBlockEntity.getContainerAt finds one after the block.
        Container entity = containerEntity(level, pos);
        return entity == null ? null : ContainerStorage.of(entity, side);
    }

    /**
     * HopperBlockEntity.getEntityContainer among the mods' entities: a live Container whose
     * box meets the block's (vanilla's box around its centre, ±0.5), a random one of several.
     * Horizon's own minecarts and chest boats are Rust's (machines.rs, entity_container_at).
     */
    static Container containerEntity(ServerLevel level, BlockPos pos) {
        AABB box = new AABB(pos);
        List<Container> found = new ArrayList<>();
        for (Entity e : HorizonEntities.of(level)) {
            if (e instanceof Container c && e.isAlive() && !e.isRemoved() && e.getBoundingBox().intersects(box)) found.add(c);
        }
        if (found.isEmpty()) return null;
        return found.size() == 1 ? found.get(0) : found.get(level.getRandom().nextInt(found.size()));
    }

    /** For the debug log: the block, or the mod's container entity there. */
    private static String what(ServerLevel level, BlockPos pos) {
        var state = level.getBlockState(pos);
        if (!state.isAir()) return state.toString();
        Container entity = containerEntity(level, pos);
        return entity instanceof Entity e ? e.getType().toShortString() : state.toString();
    }

    /**
     * The blocks the mods' container entities are in, per dimension, for the snapshot
     * (HorizonEntityRender): Horizon's hoppers and droppers ask here only where one is.
     */
    static void containers(StringBuilder out) {
        boolean firstDim = true;
        for (var level : HorizonEntities.all().entrySet()) {
            boolean first = true;
            for (Entity e : level.getValue()) {
                if (!(e instanceof Container) || !e.isAlive() || e.isRemoved()) continue;
                AABB b = e.getBoundingBox();
                // The blocks its box meets (AABB.intersects is strict: a face touching isn't).
                int x0 = Mth.floor(b.minX), y0 = Mth.floor(b.minY), z0 = Mth.floor(b.minZ);
                int x1 = Mth.ceil(b.maxX) - 1, y1 = Mth.ceil(b.maxY) - 1, z1 = Mth.ceil(b.maxZ) - 1;
                for (int x = x0; x <= x1; x++) for (int y = y0; y <= y1; y++) for (int z = z0; z <= z1; z++) {
                    if (first) {
                        if (!firstDim) out.append(',');
                        firstDim = false;
                        HorizonJson.string(level.getKey().dimension().identifier().toString(), out);
                        out.append(":[");
                    } else {
                        out.append(',');
                    }
                    first = false;
                    out.append('[').append(x).append(',').append(y).append(',').append(z).append(']');
                }
            }
            if (!first) out.append(']');
        }
    }

    /**
     * One item of the first of {@code stacks} (the hopper's slots in order, one-item SNBT, null
     * for an empty slot) that the block at {@code pos} takes through {@code face}: its index;
     * -1 when none fits, -2 when the block has no item storage there.
     */
    static int insert(ServerLevel level, BlockPos pos, int face, String[] stacks) {
        Storage<ItemVariant> target = storage(level, pos, face);
        if (target == null || !target.supportsInsertion()) return -2;
        for (int i = 0; i < stacks.length; i++) {
            ItemStack stack = HorizonServer.item(stacks[i]);
            if (stack.isEmpty()) continue;
            try (Transaction tx = Transaction.openOuter()) {
                if (target.insert(ItemVariant.of(stack), 1, tx) == 1) {
                    tx.commit();
                    if (DEBUG) Natives.log("hoppers: " + stack.getItem() + " into " + what(level, pos) + " at " + pos.toShortString());
                    return i;
                }
            }
        }
        return -1;
    }

    /**
     * One item out of the block at {@code pos} through {@code face}, the first its storage
     * offers that the hopper has room for ({@code anyRoom}: an empty slot; else a stack of
     * {@code partial}, one-item SNBT, not full yet): its SNBT; "" when none, null when the
     * block has no item storage there.
     */
    static String extract(ServerLevel level, BlockPos pos, int face, boolean anyRoom, String[] partial) {
        Storage<ItemVariant> source = storage(level, pos, face);
        if (source == null || !source.supportsExtraction()) return null;
        List<ItemStack> room = new ArrayList<>();
        for (String s : partial) {
            ItemStack stack = HorizonServer.item(s);
            if (!stack.isEmpty()) room.add(stack);
        }
        // What it holds first: a storage isn't changed while it is iterated.
        List<ItemVariant> offered = new ArrayList<>();
        for (StorageView<ItemVariant> view : source.nonEmptyViews()) {
            ItemVariant resource = view.getResource();
            if (!resource.isBlank() && !offered.contains(resource)) offered.add(resource);
        }
        for (ItemVariant resource : offered) {
            ItemStack one = resource.toStack(1);
            if (!anyRoom && room.stream().noneMatch(r -> ItemStack.isSameItemSameComponents(r, one))) continue;
            try (Transaction tx = Transaction.openOuter()) {
                if (source.extract(resource, 1, tx) == 1) {
                    tx.commit();
                    if (DEBUG) Natives.log("hoppers: " + one.getItem() + " out of " + what(level, pos) + " at " + pos.toShortString());
                    String snbt = HorizonServer.snbt(one);
                    return snbt == null ? "" : snbt;
                }
            }
        }
        return "";
    }
}
