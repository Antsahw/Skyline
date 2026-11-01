package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * The second round of vanilla actions Horizon does in Rust that mods patch (the mixin table on
 * a 91-jar pack): a player's attack on and use of Horizon's own entities, the item used in the
 * air, and a crafting result taken. Horizon's objects aren't in the mirror, so each call makes
 * a copy for it (the entity from Horizon's NBT, the stack, the crafting grid), runs what the
 * mods hook on it, and sends back what they did: stopped, took the click, removed the entity
 * (Carry On's pickup), changed the player's slots.
 */
public final class HorizonVanillaActions {
    private HorizonVanillaActions() {}

    /** HORIZON_JAVA_DEBUG=entities: each attack on and use of Horizon's entities is logged. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("entities");

    // ---- Horizon's entities: attack and use ----------------------------------------------

    private static final String ATTACK = "(Lnet/minecraft/world/entity/Entity;)V";
    private static final String INTERACT_ON =
            "(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/phys/Vec3;)Lnet/minecraft/world/InteractionResult;";

    /**
     * A player attacks ({@code use} false) or uses one of Horizon's entities, given as vanilla
     * saves it ({@code snbt}): Fabric's AttackEntityCallback / UseEntityCallback (Carry On's
     * pickup, Mob Lassos, claims), then the mods' injections at the head of Player.attack /
     * Player.interactOn, on a copy of the entity made for the call. Bits: 1 a mod took it (Horizon
     * does nothing more), 2 the arm swings, 4 the mods removed the entity (picked up: Horizon
     * removes its own, without drops). The copy is never added to a level.
     */
    public static int entity(String uuid, String dimension, String snbt, boolean use, int hand,
                             double x, double y, double z, int selected) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        if (player == null || snbt == null) return 0;
        ServerLevel level = HorizonServer.level(dimension);
        if (selected >= 0 && selected < 9) player.getInventory().setSelectedSlot(selected);
        Entity target;
        try {
            var tag = net.minecraft.nbt.TagParser.parseCompoundFully(snbt);
            target = EntityType.loadEntityRecursive(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), tag),
                    level, EntitySpawnReason.LOAD, e -> e);
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("vanilla entity " + t.getClass())) Natives.log("Java mods: an entity Horizon has couldn't be copied: " + t);
            return 0;
        }
        if (target == null) return 0;
        var interaction = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
        int[] out = {0};
        if (!use) {
            HorizonServer.fireEach("AttackEntityCallback", net.fabricmc.fabric.api.event.player.AttackEntityCallback.EVENT, l -> {
                if (out[0] != 0) return;
                out[0] = bits(l.interact(player, level, interaction, target, null));
            });
            if (out[0] == 0 && HorizonHooks.hooked("net/minecraft/world/entity/player/Player", "attack")) {
                var r = HorizonHooks.fire(player, Player.class, "attack", ATTACK, "HEAD", new Object[] {target}, null, true);
                if (r.cancelled()) out[0] = 1;
            }
        } else {
            var hit = new EntityHitResult(target, new Vec3(x, y, z));
            HorizonServer.fireEach("UseEntityCallback", net.fabricmc.fabric.api.event.player.UseEntityCallback.EVENT, l -> {
                if (out[0] != 0) return;
                out[0] = bits(l.interact(player, level, interaction, target, hit));
            });
            if (out[0] == 0 && HorizonHooks.hooked("net/minecraft/world/entity/player/Player", "interactOn")) {
                Object[] args = {target, interaction, new Vec3(x - target.getX(), y - target.getY(), z - target.getZ())};
                var r = HorizonHooks.fire(player, Player.class, "interactOn", INTERACT_ON, "HEAD", args, InteractionResult.PASS, true);
                if (r.cancelled()) out[0] = r.returnValue() instanceof InteractionResult res ? Math.max(1, bits(res)) : 1;
            }
        }
        if (target.isRemoved()) out[0] |= 4 | 1;
        if (DEBUG) Natives.log("entities: " + (use ? "use of " : "attack on ") + target + " by " + player.getName().getString()
                + " (sneaking " + player.isShiftKeyDown() + "): " + out[0]);
        HorizonServer.afterCall();
        return out[0];
    }

    private static int bits(InteractionResult r) {
        if (r == null || r == InteractionResult.PASS) return 0;
        return r instanceof InteractionResult.Success s && s.shouldSwing() ? 3 : 1;
    }

    /**
     * A vanilla entity a mod's code adds to a mirrored level (Carry On putting a carried cow
     * down): saved as vanilla saves it, for Horizon to load as its own. True when sent.
     */
    static boolean spawnVanilla(ServerLevel level, Entity entity) {
        if (!(entity instanceof net.minecraft.world.entity.Mob)) return false;
        try {
            TagValueOutput out = TagValueOutput.createWithContext(ProblemReporter.DISCARDING, level.registryAccess());
            if (!entity.save(out)) return false;
            if (DEBUG) {
                Natives.log("entities: a mod added " + entity + " to Horizon's world");
                for (StackTraceElement f : new Throwable().getStackTrace()) Natives.log("    at " + f);
            }
            Natives.spawnEntity(HorizonServer.dimensionOf(level), out.buildResult().toString());
            return true;
        } catch (Throwable t) {
            Natives.log("Java mods: a vanilla entity a mod added couldn't go to Horizon: " + t);
            return false;
        }
    }

    // ---- An item used in the air ----------------------------------------------------------

    private static final String USE = "(Lnet/minecraft/world/level/Level;Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;)Lnet/minecraft/world/InteractionResult;";
    private static final Set<String> USE_METHODS = Set.of("use", "releaseUsing", "finishUsingItem", "getUseDuration", "getUseAnimation");
    private static final Set<String> USE_FIRED = Set.of("net/minecraft/world/item/Item.use", "net/minecraft/world/item/ItemStack.use");
    private static final Map<Class<?>, Boolean> USE_ROUTED = new ConcurrentHashMap<>();

    /**
     * Whether using this vanilla item in the air is the mirror's: a mod patches its class's use
     * (or what it calls) other than at a head Horizon fires (any class's use, ItemStack.use): a bucket
     * whose use claims mods patch (Architectury, Open Parties and Claims).
     */
    public static boolean useRouted(ItemStack stack) {
        return USE_ROUTED.computeIfAbsent(stack.getItem().getClass(), c -> {
            // The heads of use on the item's classes are fired (fireUse), not routed.
            Set<String> fired = new java.util.HashSet<>(USE_FIRED);
            for (Class<?> k = c; k != null && k != Object.class; k = k.getSuperclass()) fired.add(k.getName().replace('.', '/') + ".use");
            boolean routed = HorizonMixinTable.patchedInside(c, USE_METHODS, fired)
                    || HorizonMixinTable.patchedInside(ItemStack.class, Set.of("use"), USE_FIRED);
            if (routed) Natives.log("Using " + BuiltInRegistries.ITEM.getKey(stack.getItem()) + " in the air goes to the mods (they patch its inside)");
            return routed;
        });
    }

    /**
     * The mods' injections at the head of ItemStack.use and of Item.use on the item's class
     * chain (GlitchCore's item-use event, Naturalist's bottle, Trinkets' equip), for a vanilla
     * item Horizon uses itself. The result a handler set when one cancels; else null.
     */
    public static InteractionResult fireUse(ServerLevel level, ServerPlayer player, ItemStack stack, InteractionHand hand) {
        Object[] args = {level, player, hand};
        if (HorizonHooks.hooked("net/minecraft/world/item/ItemStack", "use")) {
            var r = HorizonHooks.fire(stack, ItemStack.class, "use", USE, "HEAD", args, InteractionResult.PASS, true);
            if (r.cancelled()) return r.returnValue() instanceof InteractionResult res ? res : InteractionResult.SUCCESS;
        }
        for (Class<?> c = stack.getItem().getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            String owner = c.getName().replace('.', '/');
            if (!HorizonHooks.hooked(owner, "use")) continue;
            var r = HorizonHooks.fire(stack.getItem(), c, "use", USE, "HEAD", args, InteractionResult.PASS, true);
            if (r.cancelled()) return r.returnValue() instanceof InteractionResult res ? res : InteractionResult.SUCCESS;
        }
        return null;
    }

    // ---- A crafting result taken ----------------------------------------------------------

    private static java.lang.reflect.Field removeCount;

    /**
     * A player took {@code count} of a crafting result ({@code result}, SNBT) made by recipe
     * {@code recipe} from a grid of {@code width} ({@code grid}: SNBT, "" for empty, as it was
     * before the take): vanilla's ResultSlot.onTake on copies (Architectury's and Balm's
     * crafted events, Modonomicon's and RebornCore's hooks in checkTakeAchievements, the
     * item's onCraftedBy). The grid copy's changes are dropped (Horizon took the ingredients);
     * what the mods give the player comes back as slots.
     */
    public static void crafted(String uuid, String recipe, String result, int count, int width, String[] grid) {
        Thread.currentThread().setContextClassLoader(HorizonVanillaActions.class.getClassLoader());
        ServerPlayer player = HorizonServer.player(UUID.fromString(uuid));
        ItemStack stack = HorizonServer.item(result);
        if (DEBUG) Natives.log("crafting: " + uuid + " took " + count + " of " + result + " (" + recipe + ")");
        if (player == null || stack.isEmpty() || grid == null || width <= 0) return;
        HorizonServer.fire("ResultSlot.onTake", () -> {
            var menu = player.inventoryMenu;
            var craftSlots = new net.minecraft.world.inventory.TransientCraftingContainer(menu, width, Math.max(1, grid.length / width));
            // One of each ingredient: onTake takes one from each cell, so a remainder (a milk
            // bucket's bucket) goes back into the copy's cell, never into the player's
            // inventory (Horizon has given it already).
            for (int i = 0; i < grid.length && i < craftSlots.getContainerSize(); i++) {
                ItemStack cell = HorizonServer.item(grid[i]);
                craftSlots.setItem(i, cell.isEmpty() ? cell : cell.copyWithCount(1));
            }
            var results = new net.minecraft.world.inventory.ResultContainer();
            try {
                var key = ResourceKey.create(Registries.RECIPE, Identifier.parse(recipe));
                player.level().getServer().getRecipeManager().byKey(key).ifPresent(results::setRecipeUsed);
            } catch (RuntimeException e) {
                // No such recipe in the mirror: the take without it.
            }
            var slot = new net.minecraft.world.inventory.ResultSlot(player, craftSlots, results, 0, 0, 0);
            try {
                if (removeCount == null) {
                    removeCount = net.minecraft.world.inventory.ResultSlot.class.getDeclaredField("removeCount");
                    removeCount.setAccessible(true);
                }
                removeCount.setInt(slot, count);
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException(e);
            }
            slot.onTake(player, stack.copyWithCount(Math.max(1, count)));
            if (DEBUG) Natives.log("crafting: the mirror's inventory after: " + player.getInventory().getNonEquipmentItems());
        });
        HorizonServer.afterCall();
    }
}
