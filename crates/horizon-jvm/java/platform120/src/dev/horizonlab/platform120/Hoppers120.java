package dev.horizonlab.platform120;

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
 * Horizon's hoppers and droppers moving items into and out of a 1.20.1 mod's block: a Farmer's
 * Delight cooking pot fed from above, a mod's machine emptied from below. Horizon keeps the
 * block entity as the mod's opaque NBT, so its hopper asks here, as Fabric Transfer API's patch
 * of vanilla's ejectItems and suckInItems asks: the block's {@code ItemStorage.SIDED} on the face
 * the hopper touches (which also wraps a plain Container or WorldlyContainer a mod's block
 * entity is). One item a call, as a hopper moves.
 *
 * <p>The same as the 26.3 bridge's HorizonHoppers, on 1.20.1's names. The block entity is the
 * mirror's live one (Mirror120's begin/finish), so what moved goes back to Horizon as every
 * call's changes do.
 */
final class Hoppers120 {
    private Hoppers120() {}

    /** No storage on that face. */
    static final int NO_STORAGE = -2;
    /** A storage, and nothing moved. */
    static final int NOTHING = -1;

    private static Storage<ItemVariant> storage(ServerLevel level, BlockPos pos, int face) {
        return ItemStorage.SIDED.find(level, pos, Direction.from3DDataValue(face));
    }

    /**
     * One item of the first of {@code stacks} (the hopper's slots in order) that the block at
     * {@code pos} takes through {@code face}: its index; {@link #NOTHING} when none fits,
     * {@link #NO_STORAGE} when the block has no item storage there.
     */
    static int insert(ServerLevel level, BlockPos pos, int face, List<ItemStack> stacks) {
        Storage<ItemVariant> target = storage(level, pos, face);
        if (target == null || !target.supportsInsertion()) return NO_STORAGE;
        for (int i = 0; i < stacks.size(); i++) {
            ItemStack stack = stacks.get(i);
            if (stack.isEmpty()) continue;
            try (Transaction tx = Transaction.openOuter()) {
                if (target.insert(ItemVariant.of(stack), 1, tx) == 1) {
                    tx.commit();
                    return i;
                }
            }
        }
        return NOTHING;
    }

    /**
     * One item out of the block at {@code pos} through {@code face}: the first its storage
     * offers that the hopper has room for ({@code anyRoom}: an empty slot; else one of
     * {@code partial}, its stacks not full yet). The item; empty when none moved; null when the
     * block has no item storage there.
     */
    static ItemStack extract(ServerLevel level, BlockPos pos, int face, boolean anyRoom, List<ItemStack> partial) {
        Storage<ItemVariant> source = storage(level, pos, face);
        if (source == null || !source.supportsExtraction()) return null;
        // What it holds first: a storage isn't changed while it is iterated.
        List<ItemVariant> offered = new ArrayList<>();
        for (StorageView<ItemVariant> view : source.nonEmptyViews()) {
            ItemVariant resource = view.getResource();
            if (!resource.isBlank() && !offered.contains(resource)) offered.add(resource);
        }
        for (ItemVariant resource : offered) {
            ItemStack one = resource.toStack(1);
            // HopperBlockEntity.canMergeItems: the same item and tag, as Horizon's stacks are
            // told apart (item and damage; Horizon's others don't cross yet).
            if (!anyRoom && partial.stream().noneMatch(r -> r.getItem() == one.getItem()
                    && r.getDamageValue() == one.getDamageValue())) {
                continue;
            }
            try (Transaction tx = Transaction.openOuter()) {
                if (source.extract(resource, 1, tx) == 1) {
                    tx.commit();
                    return one;
                }
            }
        }
        return ItemStack.EMPTY;
    }
}
