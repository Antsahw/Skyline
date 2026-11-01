package dev.horizonlab.crittermod;

import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.biome.v1.BiomeModifications;
import net.fabricmc.fabric.api.biome.v1.BiomeSelectors;
import net.fabricmc.fabric.api.object.builder.v1.entity.FabricDefaultAttributeRegistry;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.SpawnEggItem;

/** The critter's type, attributes and spawn egg, as a mob mod registers them. */
public final class CritterMod implements ModInitializer {
    public static final Identifier ID = Identifier.fromNamespaceAndPath("horizon_critters", "critter");
    public static final ResourceKey<EntityType<?>> KEY = ResourceKey.create(Registries.ENTITY_TYPE, ID);
    public static final EntityType<Critter> CRITTER = Registry.register(BuiltInRegistries.ENTITY_TYPE, KEY,
            EntityType.Builder.of(Critter::new, MobCategory.CREATURE).sized(0.8F, 0.9F).build(KEY));

    /** A hostile one: it hunts players and bites (a check of mods' mobs attacking players). */
    public static final Identifier BITER_ID = Identifier.fromNamespaceAndPath("horizon_critters", "biter");
    public static final ResourceKey<EntityType<?>> BITER_KEY = ResourceKey.create(Registries.ENTITY_TYPE, BITER_ID);
    public static final EntityType<Biter> BITER = Registry.register(BuiltInRegistries.ENTITY_TYPE, BITER_KEY,
            EntityType.Builder.of(Biter::new, MobCategory.MONSTER).sized(0.8F, 0.9F).build(BITER_KEY));

    /** Its own sound (assets/horizon_critters/sounds.json) and particle type (particles/sparkle.json). */
    public static final net.minecraft.sounds.SoundEvent SQUEAK = Registry.register(BuiltInRegistries.SOUND_EVENT,
            Identifier.fromNamespaceAndPath("horizon_critters", "critter.squeak"),
            net.minecraft.sounds.SoundEvent.createVariableRangeEvent(Identifier.fromNamespaceAndPath("horizon_critters", "critter.squeak")));
    public static final net.minecraft.core.particles.SimpleParticleType SPARKLE = Registry.register(BuiltInRegistries.PARTICLE_TYPE,
            Identifier.fromNamespaceAndPath("horizon_critters", "sparkle"),
            net.fabricmc.fabric.api.particle.v1.FabricParticleTypes.simple());

    @Override
    public void onInitialize() {
        FabricDefaultAttributeRegistry.register(CRITTER, Mob.createMobAttributes()
                .add(Attributes.MAX_HEALTH, 8.0)
                .add(Attributes.MOVEMENT_SPEED, 0.25));
        FabricDefaultAttributeRegistry.register(BITER, Mob.createMobAttributes()
                .add(Attributes.MAX_HEALTH, 10.0)
                .add(Attributes.MOVEMENT_SPEED, 0.3)
                .add(Attributes.ATTACK_DAMAGE, 3.0)
                .add(Attributes.FOLLOW_RANGE, 16.0));
        ResourceKey<Item> egg = ResourceKey.create(Registries.ITEM,
                Identifier.fromNamespaceAndPath("horizon_critters", "critter_spawn_egg"));
        Registry.register(BuiltInRegistries.ITEM, egg, new SpawnEggItem(new Item.Properties().spawnEgg(CRITTER).setId(egg)));
        // Natural spawns in every overworld biome, in groups of 2 to 4.
        BiomeModifications.addSpawn(BiomeSelectors.foundInOverworld(), MobCategory.CREATURE, CRITTER, 60, 2, 4);
    }
}
