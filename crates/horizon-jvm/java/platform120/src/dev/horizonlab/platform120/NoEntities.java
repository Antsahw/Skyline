package dev.horizonlab.platform120;

import java.util.UUID;
import java.util.function.Consumer;
import net.minecraft.util.AbortableIterationConsumer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.entity.EntityTypeTest;
import net.minecraft.world.level.entity.LevelEntityGetter;
import net.minecraft.world.phys.AABB;

/**
 * The entities of a mirror level: none (docs/java-bridge-120.md, phase 3). Horizon owns this
 * world's entities, and showing them to 1.20.1 code is a later phase; until then a mod asking
 * what is in a box is told nothing is.
 *
 * <p>An empty answer rather than no answer at all: a mirror level's entity manager is never
 * made, so {@code ServerLevel.getEntities()} would be null, and everything that asks would
 * throw. Placing a block asks ({@code BlockItem.place} -> {@code isUnobstructed} ->
 * {@code getEntities}), so nothing a mod placed could be placed through the mirror.
 *
 * <p>What this costs while entities are Horizon's: a mod placing a block does not notice an
 * entity standing where it goes, and a mod looking for entities finds none. Both are wrong in
 * the same direction as the rest of the mirror -- the world the mods see holds blocks only.
 */
public final class NoEntities implements LevelEntityGetter<Entity> {
    public static final NoEntities INSTANCE = new NoEntities();

    private NoEntities() {}

    @Override
    public Entity get(int id) {
        return null;
    }

    @Override
    public Entity get(UUID uuid) {
        return null;
    }

    @Override
    public Iterable<Entity> getAll() {
        return java.util.List.of();
    }

    @Override
    public <U extends Entity> void get(EntityTypeTest<Entity, U> test,
            AbortableIterationConsumer<U> consumer) {}

    @Override
    public void get(AABB box, Consumer<Entity> consumer) {}

    @Override
    public <U extends Entity> void get(EntityTypeTest<Entity, U> test, AABB box,
            AbortableIterationConsumer<U> consumer) {}
}
