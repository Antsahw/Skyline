package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.List;

import net.fabricmc.fabric.api.transfer.v1.item.ItemStorage;
import net.fabricmc.fabric.api.transfer.v1.item.ItemVariant;
import net.fabricmc.fabric.api.transfer.v1.storage.Storage;
import net.fabricmc.fabric.api.transfer.v1.storage.StorageView;
import net.fabricmc.fabric.api.transfer.v1.transaction.Transaction;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;

/**
 * Horizon's hoppers (Rust) moving items into and out of a Java mod's block: Farmer's Delight's
 * cooking pot, Energized Power's machines, mods' chests. Horizon's world has no block entity
 * there (the mod's lives in Java: HorizonBlockEntities.LIVE), so its hopper asks here, as
 * Fabric Transfer API's patch of vanilla's ejectItems/suckInItems asks: the block's
 * ItemStorage.SIDED on the face the hopper touches (which also wraps a plain vanilla Container
 * or WorldlyContainer a mod's block entity is). One item a call, as a hopper moves.
 */
final class HorizonHoppers {
    private HorizonHoppers() {}

    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("hoppers");

    private static Storage<ItemVariant> storage(ServerLevel level, BlockPos pos, int face) {
        return ItemStorage.SIDED.find(level, pos, Direction.from3DDataValue(face));
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
                    if (DEBUG) Natives.log("hoppers: " + stack.getItem() + " into " + level.getBlockState(pos) + " at " + pos.toShortString());
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
                    if (DEBUG) Natives.log("hoppers: " + one.getItem() + " out of " + level.getBlockState(pos) + " at " + pos.toShortString());
                    String snbt = HorizonServer.snbt(one);
                    return snbt == null ? "" : snbt;
                }
            }
        }
        return "";
    }
}
