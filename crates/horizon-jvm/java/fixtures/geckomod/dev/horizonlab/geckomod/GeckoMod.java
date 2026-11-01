package dev.horizonlab.geckomod;

import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.MobCategory;

/** The bobber's type and attributes. */
public final class GeckoMod implements ModInitializer {
    public static final Identifier ID = Identifier.fromNamespaceAndPath("horizon_gecko", "bobber");
    public static final ResourceKey<EntityType<?>> KEY = ResourceKey.create(Registries.ENTITY_TYPE, ID);
    public static final EntityType<Bobber> BOBBER = Registry.register(BuiltInRegistries.ENTITY_TYPE, KEY,
            EntityType.Builder.of(Bobber::new, MobCategory.CREATURE).sized(0.8F, 1.2F).build(KEY));

    @Override
    public void onInitialize() {
        FabricDefaultAttributeRegistry.register(BOBBER, Mob.createMobAttributes());
    }
}
