package dev.horizonlab.platform120;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;
import java.util.TreeSet;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.ArmorItem;
import net.minecraft.world.item.ArmorMaterial;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.DyeableLeatherItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TieredItem;
import net.minecraft.world.item.crafting.Ingredient;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockSetType;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.block.state.properties.WoodType;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * The 1.20.1 mods' blocks and items, for Horizon's content (docs/java-bridge-120.md, phase 2): the
 * oracle facts horizon-port-pack's carry-over reads (oracle.rs), as the 26.3 platform's
 * HorizonContent writes them, from 1.20.1's registries in the bridge's 1.20.1 runtime.
 *
 * <p>Every block and item outside {@code minecraft} (and the loaders' own), with vanilla's blocks
 * beside them so templates can be matched, and every block and item tag as the game's data packs
 * make it (Tags120: vanilla's, Fabric API's convention tags, the mods'), which gives the
 * convention tags the mods' recipes name their members. What needs a level (drops, placement) is
 * left out until the 1.20.1 mirror has one: those facts are optional, and the carry-over takes
 * the mods' own loot tables from their jars. Items' numbers are written in 1.21.1's component
 * form, which the carry-over upgrades from.
 */
public final class Content120 {
    private Content120() {}

    /** Namespaces that are no mod's content: vanilla, the loaders', conventions, Horizon's. */
    private static final Set<String> NOT_CONTENT = Set.of("minecraft", "c", "horizon", "fabric", "neoforge", "forge");

    /** Writes the facts to {@code out}; the mods' namespaces it found, comma separated. */
    public static String exportFacts(String out) throws IOException {
        Set<String> namespaces = new TreeSet<>();
        for (ResourceLocation id : BuiltInRegistries.BLOCK.keySet()) namespaces.add(id.getNamespace());
        for (ResourceLocation id : BuiltInRegistries.ITEM.keySet()) namespaces.add(id.getNamespace());
        namespaces.removeAll(NOT_CONTENT);
        Path path = Path.of(out);
        if (path.getParent() != null) Files.createDirectories(path.getParent());
        try (Writer w = Files.newBufferedWriter(path, StandardCharsets.UTF_8); JsonWriter json = new JsonWriter(w)) {
            new Export(namespaces, json).run();
        }
        return String.join(",", namespaces);
    }

    private static final class Export {
        final Set<String> namespaces;
        final JsonWriter json;

        Export(Set<String> namespaces, JsonWriter json) {
            this.namespaces = namespaces;
            this.json = json;
        }

        boolean ours(ResourceLocation id) {
            return namespaces.contains(id.getNamespace());
        }

        void run() throws IOException {
            json.beginObject();
            json.name("schema").value(1);
            json.name("minecraft").value("1.20.1");
            json.name("namespaces");
            json.beginArray();
            for (String n : namespaces) json.value(n);
            json.endArray();
            json.name("tools");
            json.beginArray();
            json.endArray();
            blocks();
            json.name("vanilla_items");
            json.beginArray();
            for (Item item : BuiltInRegistries.ITEM) {
                ResourceLocation id = BuiltInRegistries.ITEM.getKey(item);
                if (id.getNamespace().equals("minecraft")) json.value(id.toString());
            }
            json.endArray();
            items();
            creativeTabs();
            mobEffects();
            trades();
            json.name("tags");
            json.beginObject();
            Tags120.Loaded tags = Tags120.load();
            tags(json, "block", tags.blocks());
            tags(json, "item", tags.items());
            json.endObject();
            json.endObject();
        }

        /**
         * The mods' own creative tabs (Farmer's Delight's): their id, title (its translation
         * key, or the text of a literal one), icon and items, as the tab's own code fills it
         * (CreativeModeTabs.tryRebuildTabContents, with the server's registries). A tab whose
         * contents can't be made here is left out: its items are still in the search tab.
         */
        /**
         * The trades the mods add to villagers and the wandering trader (Fabric's
         * TradeOfferHelper puts their listings into VillagerTrades' tables, beside vanilla's):
         * {@code [{"set": "farmer/level_1" | "wandering_trader/common", "offer": {...}}]}, each
         * listing asked for its offer once, as 26.3's villager_trade has it (`wants`,
         * `additional_wants`, `gives`, `max_uses`, `xp`, `reputation_discount`). A listing whose
         * offer is random (a random item, a random price) keeps the one it gave; one that needs
         * a villager or a level to answer, or gives an item with data, is left out.
         */
        void trades() throws IOException {
            json.name("trades");
            json.beginArray();
            var random = net.minecraft.util.RandomSource.create(1201L);
            for (var entry : net.minecraft.world.entity.npc.VillagerTrades.TRADES.entrySet()) {
                ResourceLocation profession = BuiltInRegistries.VILLAGER_PROFESSION.getKey(entry.getKey());
                // A mod's own profession has no villagers in Horizon.
                if (profession == null || !profession.getNamespace().equals("minecraft")) continue;
                for (var level : entry.getValue().int2ObjectEntrySet()) {
                    String set = profession.getPath() + "/level_" + level.getIntKey();
                    for (var listing : level.getValue()) offer(set, listing, random);
                }
            }
            for (var level : net.minecraft.world.entity.npc.VillagerTrades.WANDERING_TRADER_TRADES.int2ObjectEntrySet()) {
                String set = "wandering_trader/" + (level.getIntKey() == 1 ? "common" : "uncommon");
                for (var listing : level.getValue()) offer(set, listing, random);
            }
            json.endArray();
        }

        private void offer(String set, net.minecraft.world.entity.npc.VillagerTrades.ItemListing listing,
                net.minecraft.util.RandomSource random) throws IOException {
            String owner = listing.getClass().getName();
            // Vanilla's own listings are 26.3's data already.
            if (owner.startsWith("net.minecraft.")) return;
            net.minecraft.world.item.trading.MerchantOffer offer;
            try {
                offer = listing.getOffer(null, random);
            } catch (Throwable t) {
                return;
            }
            if (offer == null || offer.getResult().isEmpty() || offer.getResult().hasTag()
                    || offer.getBaseCostA().hasTag() || offer.getCostB().hasTag()) {
                return;
            }
            json.beginObject();
            json.name("set").value(set);
            json.name("offer");
            json.beginObject();
            json.name("wants");
            stack(offer.getBaseCostA());
            if (!offer.getCostB().isEmpty()) {
                json.name("additional_wants");
                stack(offer.getCostB());
            }
            json.name("gives");
            stack(offer.getResult());
            json.name("max_uses").value(offer.getMaxUses());
            json.name("xp").value(offer.getXp());
            json.name("reputation_discount").value(offer.getPriceMultiplier());
            json.endObject();
            json.endObject();
        }

        private void stack(ItemStack stack) throws IOException {
            json.beginObject();
            json.name("id").value(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString());
            if (stack.getCount() != 1) json.name("count").value(stack.getCount());
            json.endObject();
        }

        /**
         * The mods' own effects (Farmer's Delight's Comfort and Nourishment): as Horizon's content
         * declares one, `{"id", "category", "color"}`.
         */
        void mobEffects() throws IOException {
            json.name("mob_effects");
            json.beginArray();
            for (net.minecraft.world.effect.MobEffect effect : BuiltInRegistries.MOB_EFFECT) {
                ResourceLocation id = BuiltInRegistries.MOB_EFFECT.getKey(effect);
                if (id == null || !ours(id)) continue;
                json.beginObject();
                json.name("id").value(id.toString());
                json.name("category").value(effect.getCategory().name().toLowerCase(java.util.Locale.ROOT));
                json.name("color").value(effect.getColor() & 0xFFFFFF);
                json.endObject();
            }
            json.endArray();
        }

        void creativeTabs() throws IOException {
            json.name("creative_tabs");
            json.beginArray();
            try {
                net.minecraft.world.item.CreativeModeTabs.tryRebuildTabContents(
                        net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS, false, Mirror120.registries());
            } catch (RuntimeException | LinkageError e) {
                System.out.println("[horizon-platform120] the creative tabs' contents weren't made: " + e);
            }
            for (var tab : BuiltInRegistries.CREATIVE_MODE_TAB) {
                ResourceLocation id = BuiltInRegistries.CREATIVE_MODE_TAB.getKey(tab);
                if (id == null || !ours(id) || tab.getType() != net.minecraft.world.item.CreativeModeTab.Type.CATEGORY) continue;
                java.util.Collection<net.minecraft.world.item.ItemStack> items;
                String icon;
                try {
                    items = tab.getDisplayItems();
                    icon = BuiltInRegistries.ITEM.getKey(tab.getIconItem().getItem()).toString();
                } catch (RuntimeException | LinkageError e) {
                    System.out.println("[horizon-platform120] the creative tab " + id + " couldn't be read: " + e);
                    continue;
                }
                json.beginObject();
                json.name("id").value(id.toString());
                var title = tab.getDisplayName();
                if (title.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t) {
                    json.name("title_key").value(t.getKey());
                }
                json.name("title").value(title.getString());
                json.name("icon").value(icon);
                json.name("items");
                json.beginArray();
                java.util.Set<String> seen = new java.util.LinkedHashSet<>();
                for (var stack : items) seen.add(BuiltInRegistries.ITEM.getKey(stack.getItem()).toString());
                for (String item : seen) json.value(item);
                json.endArray();
                json.endObject();
            }
            json.endArray();
        }

        void blocks() throws IOException {
            json.name("blocks");
            json.beginObject();
            for (Block block : BuiltInRegistries.BLOCK) {
                ResourceLocation id = BuiltInRegistries.BLOCK.getKey(block);
                if (!ours(id) && !id.getNamespace().equals("minecraft")) continue;
                json.name(id.toString());
                json.beginObject();
                json.name("class");
                json.beginArray();
                for (Class<?> c = block.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
                    json.value(Names.mojang(c));
                }
                json.endArray();
                json.name("properties");
                json.beginObject();
                for (Property<?> p : block.getStateDefinition().getProperties()) {
                    json.name(p.getName());
                    json.beginArray();
                    for (Object v : p.getPossibleValues()) json.value(name(p, v));
                    json.endArray();
                }
                json.endObject();
                json.name("default").value(stateKey(block.defaultBlockState()));
                json.name("item").value(BuiltInRegistries.ITEM.getKey(block.asItem()).toString());
                json.name("explosion_resistance").value(block.getExplosionResistance());
                json.name("friction").value(block.getFriction());
                json.name("speed_factor").value(block.getSpeedFactor());
                json.name("jump_factor").value(block.getJumpFactor());
                blockSetType(block);
                json.name("states");
                json.beginObject();
                for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                    json.name(stateKey(state));
                    json.beginObject();
                    json.name("hardness").value(state.getDestroySpeed(EmptyBlockGetter.INSTANCE, BlockPos.ZERO));
                    json.name("emission").value(state.getLightEmission());
                    json.name("requires_tool").value(state.requiresCorrectToolForDrops());
                    json.name("random_ticks").value(state.isRandomlyTicking());
                    json.name("can_occlude").value(state.canOcclude());
                    if (state.getRenderShape() == RenderShape.INVISIBLE) json.name("invisible").value(true);
                    json.name("replaceable").value(state.canBeReplaced());
                    var color = state.getMapColor(EmptyBlockGetter.INSTANCE, BlockPos.ZERO);
                    json.name("map_color").value(color.col);
                    json.name("map_color_id").value(color.id);
                    sound(state.getSoundType());
                    json.name("outline");
                    boxes(state.getShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO));
                    json.name("collision");
                    boxes(state.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO));
                    // The faces it hides beside it (Block.shouldRenderFace): none when it can't occlude.
                    json.name("occlusion");
                    boxes(state.canOcclude() ? state.getOcclusionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO) : Shapes.empty());
                    json.endObject();
                }
                json.endObject();
                json.endObject();
            }
            json.endObject();
        }

        /** The BlockSetType (doors, trapdoors, buttons, plates) and WoodType (fence gates) a block keeps. */
        void blockSetType(Block block) throws IOException {
            String setType = null, woodType = null;
            Boolean byHand = null;
            for (Class<?> c = block.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
                for (var f : c.getDeclaredFields()) {
                    try {
                        if (f.getType() == BlockSetType.class && setType == null) {
                            f.setAccessible(true);
                            var t = (BlockSetType) f.get(block);
                            if (t != null) {
                                setType = t.name();
                                byHand = t.canOpenByHand();
                            }
                        } else if (f.getType() == WoodType.class && woodType == null) {
                            f.setAccessible(true);
                            var t = (WoodType) f.get(block);
                            if (t != null) woodType = t.name();
                        }
                    } catch (ReflectiveOperationException | RuntimeException e) {
                        // Not readable: left out.
                    }
                }
            }
            if (setType != null) {
                json.name("block_set_type");
                json.beginObject();
                json.name("name").value(setType);
                json.name("can_open_by_hand").value(byHand);
                json.endObject();
            }
            if (woodType != null) json.name("wood_type").value(woodType);
        }

        void sound(SoundType s) throws IOException {
            json.name("sound");
            json.beginObject();
            json.name("volume").value(s.getVolume());
            json.name("pitch").value(s.getPitch());
            event("break", s.getBreakSound());
            event("step", s.getStepSound());
            event("place", s.getPlaceSound());
            event("hit", s.getHitSound());
            event("fall", s.getFallSound());
            json.endObject();
        }

        void event(String name, SoundEvent e) throws IOException {
            json.name(name).value(e.getLocation().toString());
        }

        void boxes(VoxelShape shape) throws IOException {
            json.beginArray();
            for (AABB b : shape.toAabbs()) {
                json.beginArray();
                json.value(b.minX).value(b.minY).value(b.minZ).value(b.maxX).value(b.maxY).value(b.maxZ);
                json.endArray();
            }
            json.endArray();
        }

        void items() throws IOException {
            json.name("items");
            json.beginObject();
            for (Item item : BuiltInRegistries.ITEM) {
                ResourceLocation id = BuiltInRegistries.ITEM.getKey(item);
                if (!ours(id)) continue;
                ItemStack stack = new ItemStack(item);
                json.name(id.toString());
                json.beginObject();
                json.name("class").value(Names.mojang(item.getClass()));
                json.name("max_stack_size").value(stack.getMaxStackSize());
                json.name("rarity").value(stack.getRarity().name().toLowerCase(java.util.Locale.ROOT));
                if (item instanceof BlockItem bi) {
                    json.name("block").value(BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString());
                }
                FoodProperties food = item.getFoodProperties();
                // 1.21's saturation is absolute: nutrition x 1.20.1's modifier x 2.
                float saturation = food == null ? 0 : food.getNutrition() * food.getSaturationModifier() * 2.0f;
                if (food != null) {
                    json.name("food");
                    json.beginObject();
                    json.name("nutrition").value(food.getNutrition());
                    json.name("saturation").value(saturation);
                    json.endObject();
                    // Eating it as 26.3's consumable: how long, eaten or drunk, and the effects
                    // its food gives (FoodProperties.effects: an instance and its chance).
                    json.name("consume");
                    json.beginObject();
                    json.name("seconds").value(stack.getUseDuration() / 20.0f);
                    json.name("animation").value(stack.getUseAnimation().name().toLowerCase(java.util.Locale.ROOT));
                    // What eating leaves: Farmer's Delight's stews their bowl (the crafting
                    // remainder its ConsumableItem hands back), a BowlFoodItem its bowl.
                    Item left = item.getCraftingRemainingItem();
                    if (left == null && item instanceof net.minecraft.world.item.BowlFoodItem) {
                        left = net.minecraft.world.item.Items.BOWL;
                    }
                    if (left != null) json.name("remainder").value(BuiltInRegistries.ITEM.getKey(left).toString());
                    json.name("effects");
                    json.beginArray();
                    for (var pair : food.getEffects()) {
                        var instance = pair.getFirst();
                        ResourceLocation effect = BuiltInRegistries.MOB_EFFECT.getKey(instance.getEffect());
                        if (effect == null) continue;
                        json.beginObject();
                        json.name("id").value(effect.toString());
                        json.name("duration").value(instance.getDuration());
                        json.name("amplifier").value(instance.getAmplifier());
                        json.name("probability").value(pair.getSecond());
                        json.endObject();
                    }
                    json.endArray();
                    json.endObject();
                }
                // The item's own numbers as 1.21.1's default components.
                json.name("components");
                json.beginObject();
                json.name("minecraft:max_stack_size").value(stack.getMaxStackSize());
                json.name("minecraft:rarity").value(stack.getRarity().name().toLowerCase(java.util.Locale.ROOT));
                if (item.getMaxDamage() > 0) {
                    json.name("minecraft:max_damage").value(item.getMaxDamage());
                    json.name("minecraft:damage").value(0);
                }
                if (item.isFireResistant()) json.name("minecraft:fire_resistant").beginObject().endObject();
                if (food != null) {
                    json.name("minecraft:food");
                    json.beginObject();
                    json.name("nutrition").value(food.getNutrition());
                    json.name("saturation").value(saturation);
                    if (food.canAlwaysEat()) json.name("can_always_eat").value(true);
                    if (food.isFastFood()) json.name("eat_seconds").value(0.8f);
                    json.endObject();
                }
                json.endObject();
                json.name("enchantment_value").value(item.getEnchantmentValue());
                Ingredient repair = null;
                if (item instanceof TieredItem tiered) {
                    repair = tiered.getTier().getRepairIngredient();
                } else if (item instanceof ArmorItem armor) {
                    repair = armor.getMaterial().getRepairIngredient();
                }
                if (repair != null) {
                    json.name("repair_items");
                    json.beginArray();
                    try {
                        for (ItemStack s : repair.getItems()) json.value(BuiltInRegistries.ITEM.getKey(s.getItem()).toString());
                    } catch (RuntimeException e) {
                        // An ingredient of a tag: tags aren't bound before data packs load.
                    }
                    json.endArray();
                }
                if (item instanceof ArmorItem armor) armor(armor);
                json.endObject();
            }
            json.endObject();
        }

        /** An armour piece as 1.21.1 describes it: slot, material, its numbers, its layer textures. */
        void armor(ArmorItem armor) throws IOException {
            ArmorMaterial material = armor.getMaterial();
            String name = material.getName();
            // Fabric API's 1.20.1 armour textures: "ns:name" -> ns:textures/models/armor/name_layer_1.png.
            String ns = "minecraft", path = name;
            int colon = name.indexOf(':');
            if (colon >= 0) {
                ns = name.substring(0, colon);
                path = name.substring(colon + 1);
            }
            json.name("armor");
            json.beginObject();
            json.name("slot").value(armor.getEquipmentSlot().getName());
            json.name("material").value(ns + ":" + path);
            json.name("defense").value(armor.getDefense());
            json.name("toughness").value(armor.getToughness());
            json.name("knockback_resistance").value(material.getKnockbackResistance());
            json.name("equip_sound").value(material.getEquipSound().getLocation().toString());
            json.name("layers");
            json.beginArray();
            json.beginObject();
            json.name("texture").value(ns + ":textures/models/armor/" + path + "_layer_1.png");
            json.name("texture_legs").value(ns + ":textures/models/armor/" + path + "_layer_2.png");
            json.name("dyeable").value(armor instanceof DyeableLeatherItem);
            json.endObject();
            json.endArray();
            json.endObject();
        }

        static void tags(JsonWriter json, String kind, java.util.Map<ResourceLocation, java.util.Collection<ResourceLocation>> tags)
                throws IOException {
            json.name(kind);
            json.beginObject();
            for (var e : new java.util.TreeMap<>(tags).entrySet()) {
                json.name(e.getKey().toString());
                json.beginArray();
                for (ResourceLocation m : new TreeSet<>(e.getValue())) json.value(m.toString());
                json.endArray();
            }
            json.endObject();
        }

        static String stateKey(BlockState state) {
            StringBuilder b = new StringBuilder();
            for (Property<?> p : state.getProperties()) {
                if (b.length() > 0) b.append(',');
                b.append(p.getName()).append('=').append(name(p, state.getValue(p)));
            }
            return b.toString();
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        static String name(Property p, Object v) {
            return p.getName((Comparable) v);
        }
    }
}
