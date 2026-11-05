package dev.horizonlab.bridge112;

import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import net.minecraft.block.Block;
import net.minecraft.block.state.IBlockState;
import net.minecraft.creativetab.CreativeTabs;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.util.NonNullList;
import net.minecraft.util.ResourceLocation;
import net.minecraft.world.DimensionType;
import net.minecraftforge.common.DimensionManager;
import net.minecraftforge.fluids.Fluid;
import net.minecraftforge.fluids.FluidRegistry;
import net.minecraftforge.fml.common.registry.ForgeRegistries;
import net.minecraftforge.registries.ForgeRegistry;
import net.minecraftforge.registries.IForgeRegistry;
import net.minecraftforge.registries.IForgeRegistryEntry;

/**
 * Phase 1's facts about the loaded game (docs/java-bridge-112.md, "Mapping 1.12 to 26.3"): every
 * block with its stored states (the distinct {@code getStateFromMeta(0..15)}), every item with the
 * stacks it lists (subtypes by meta), fluids, entities, biomes, dimensions, with FML's numeric
 * ids. One JSON object; runs on the client thread (display names need the client's language).
 */
final class Report112 {
    private Report112() {}

    static String build(Map<String, String> mods, long sinceLoadMs) {
        JsonObject out = new JsonObject();
        out.addProperty("since_load_ms", sinceLoadMs);
        JsonObject modList = new JsonObject();
        mods.forEach(modList::addProperty);
        out.add("mods", modList);
        out.add("blocks", blocks());
        out.add("items", items());
        JsonArray fluids = new JsonArray();
        for (Map.Entry<String, Fluid> e : FluidRegistry.getRegisteredFluids().entrySet()) {
            JsonObject f = new JsonObject();
            f.addProperty("id", e.getKey());
            f.addProperty("block", e.getValue().getBlock() == null ? null : String.valueOf(e.getValue().getBlock().getRegistryName()));
            f.addProperty("gaseous", e.getValue().isGaseous());
            f.addProperty("temperature", e.getValue().getTemperature());
            fluids.add(f);
        }
        out.add("fluids", fluids);
        out.add("entities", ids(ForgeRegistries.ENTITIES));
        out.add("biomes", ids(ForgeRegistries.BIOMES));
        JsonArray dims = new JsonArray();
        for (Integer id : DimensionManager.getStaticDimensionIDs()) {
            JsonObject d = new JsonObject();
            d.addProperty("id", id);
            DimensionType type = DimensionManager.getProviderType(id);
            d.addProperty("type", type.getName());
            dims.add(d);
        }
        out.add("dimensions", dims);
        net.minecraft.client.Minecraft mc = net.minecraft.client.Minecraft.getMinecraft();
        JsonObject client = new JsonObject();
        client.addProperty("language", mc.getLanguageManager().getCurrentLanguage().getLanguageCode());
        JsonArray domains = new JsonArray();
        for (String d : new java.util.TreeSet<>(mc.getResourceManager().getResourceDomains())) domains.add(d);
        client.add("resource_domains", domains);
        out.add("client", client);
        return out.toString();
    }

    private static JsonArray blocks() {
        ForgeRegistry<Block> reg = (ForgeRegistry<Block>) ForgeRegistries.BLOCKS;
        JsonArray out = new JsonArray();
        for (Block block : reg.getValuesCollection()) {
            JsonObject b = new JsonObject();
            b.addProperty("id", String.valueOf(block.getRegistryName()));
            b.addProperty("num", reg.getID(block));
            b.addProperty("class", block.getClass().getName());
            JsonArray states = new JsonArray();
            Set<String> seen = new LinkedHashSet<>();
            for (int meta = 0; meta < 16; meta++) {
                IBlockState state;
                try {
                    state = block.getStateFromMeta(meta);
                } catch (RuntimeException e) {
                    continue; // some blocks throw for metas they never store
                }
                String text = state.toString();
                if (!seen.add(text)) continue;
                JsonObject s = new JsonObject();
                s.addProperty("state", text);
                try {
                    s.addProperty("meta", block.getMetaFromState(state));
                    s.addProperty("light", state.getLightValue());
                    s.addProperty("opaque", state.isOpaqueCube());
                } catch (RuntimeException e) {
                    s.addProperty("error", e.toString()); // a mod's own state code failing on its own states
                }
                states.add(s);
            }
            b.add("states", states);
            b.addProperty("all_states", block.getBlockState().getValidStates().size());
            out.add(b);
        }
        return out;
    }

    private static JsonArray items() {
        ForgeRegistry<Item> reg = (ForgeRegistry<Item>) ForgeRegistries.ITEMS;
        JsonArray out = new JsonArray();
        for (Item item : reg.getValuesCollection()) {
            JsonObject i = new JsonObject();
            i.addProperty("id", String.valueOf(item.getRegistryName()));
            i.addProperty("num", reg.getID(item));
            i.addProperty("subtypes", item.getHasSubtypes());
            i.addProperty("max_damage", item.getMaxDamage());
            i.addProperty("stack", item.getItemStackLimit());
            Block block = Block.getBlockFromItem(item);
            if (block != null && block != net.minecraft.init.Blocks.AIR) i.addProperty("block", String.valueOf(block.getRegistryName()));
            CreativeTabs tab = item.getCreativeTab();
            if (tab != null) i.addProperty("tab", tab.getTabLabel());
            NonNullList<ItemStack> stacks = NonNullList.create();
            try {
                item.getSubItems(CreativeTabs.SEARCH, stacks);
            } catch (RuntimeException e) {
                i.addProperty("sub_items_error", e.toString());
            }
            JsonArray variants = new JsonArray();
            for (ItemStack stack : stacks) {
                JsonObject v = new JsonObject();
                v.addProperty("meta", stack.getMetadata());
                if (stack.getTagCompound() != null) v.addProperty("nbt", stack.getTagCompound().toString());
                try {
                    v.addProperty("key", stack.getTranslationKey());
                    v.addProperty("name", stack.getDisplayName());
                } catch (RuntimeException e) {
                    v.addProperty("error", e.toString());
                }
                variants.add(v);
            }
            i.add("variants", variants);
            out.add(i);
        }
        return out;
    }

    private static <T extends IForgeRegistryEntry<T>> JsonArray ids(IForgeRegistry<T> registry) {
        JsonArray out = new JsonArray();
        ForgeRegistry<T> reg = (ForgeRegistry<T>) registry;
        for (Map.Entry<ResourceLocation, T> e : reg.getEntries()) {
            JsonObject o = new JsonObject();
            o.addProperty("id", e.getKey().toString());
            o.addProperty("num", reg.getID(e.getValue()));
            out.add(o);
        }
        return out;
    }
}
