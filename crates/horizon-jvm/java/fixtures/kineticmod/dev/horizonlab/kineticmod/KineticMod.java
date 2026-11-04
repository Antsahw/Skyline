package dev.horizonlab.kineticmod;

import net.fabricmc.api.ModInitializer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityTicker;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;

/** A shaft whose block entity turns on the server (its Speed and Angle saved, as Create's are). */
public final class KineticMod implements ModInitializer {
    static Identifier id(String path) {
        return Identifier.fromNamespaceAndPath("horizon_kinetic", path);
    }

    static final ResourceKey<Block> BLOCK_KEY = ResourceKey.create(Registries.BLOCK, id("shaft"));
    public static final Block SHAFT = Registry.register(BuiltInRegistries.BLOCK, BLOCK_KEY,
            new ShaftBlock(BlockBehaviour.Properties.of().strength(1.0F).noOcclusion().setId(BLOCK_KEY)));
    public static final BlockEntityType<ShaftBlockEntity> SHAFT_ENTITY = Registry.register(
            BuiltInRegistries.BLOCK_ENTITY_TYPE, id("shaft"), new BlockEntityType<>(ShaftBlockEntity::new, java.util.Set.of(SHAFT)));

    /** The entity classes whose synced data was made (SynchedDataBuilderMixin), as Create keys its accessors. */
    public static final java.util.Set<Class<?>> DATA_CLASSES = java.util.concurrent.ConcurrentHashMap.newKeySet();

    @Override
    public void onInitialize() {
        ResourceKey<Item> item = ResourceKey.create(Registries.ITEM, id("shaft"));
        Registry.register(BuiltInRegistries.ITEM, item, new BlockItem(SHAFT, new Item.Properties().setId(item)));
    }

    public static final class ShaftBlock extends Block implements EntityBlock {
        ShaftBlock(BlockBehaviour.Properties properties) {
            super(properties);
        }

        @Override
        public BlockEntity newBlockEntity(BlockPos pos, BlockState state) {
            return new ShaftBlockEntity(pos, state);
        }

        @Override
        @SuppressWarnings("unchecked")
        public <T extends BlockEntity> BlockEntityTicker<T> getTicker(Level level, BlockState state, BlockEntityType<T> type) {
            return level.isClientSide() || type != SHAFT_ENTITY ? null
                    : (BlockEntityTicker<T>) (BlockEntityTicker<ShaftBlockEntity>) (l, pos, s, be) -> be.angle = (be.angle + be.speed) % 360F;
        }
    }

    public static final class ShaftBlockEntity extends BlockEntity {
        public float speed = 16F;
        public float angle;

        public ShaftBlockEntity(BlockPos pos, BlockState state) {
            super(SHAFT_ENTITY, pos, state);
        }

        @Override
        protected void saveAdditional(ValueOutput out) {
            super.saveAdditional(out);
            out.putFloat("Speed", speed);
            out.putFloat("Angle", angle);
        }

        @Override
        protected void loadAdditional(ValueInput in) {
            super.loadAdditional(in);
            speed = in.getFloatOr("Speed", 16F);
            angle = in.getFloatOr("Angle", 0F);
        }
    }
}
