package dev.horizonlab.cartmod;

import net.fabricmc.api.ModInitializer;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.vehicle.minecart.MinecartChest;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/** A barrel minecart: a mod's entity that is a Container (vanilla's chest minecart, its own type). */
public final class CartMod implements ModInitializer {
    public static final ResourceKey<EntityType<?>> KEY = ResourceKey.create(Registries.ENTITY_TYPE,
            Identifier.fromNamespaceAndPath("horizon_carts", "barrel_cart"));
    public static final EntityType<BarrelCart> BARREL_CART = Registry.register(BuiltInRegistries.ENTITY_TYPE, KEY,
            EntityType.Builder.<BarrelCart>of(BarrelCart::new, MobCategory.MISC).sized(0.98F, 0.7F).build(KEY));

    @Override
    public void onInitialize() {}

    public static final class BarrelCart extends MinecartChest {
        public BarrelCart(EntityType<? extends MinecartChest> type, Level level) {
            super(type, level);
        }

        @Override
        public BlockState getDefaultDisplayBlockState() {
            return Blocks.BARREL.defaultBlockState();
        }
    }
}
