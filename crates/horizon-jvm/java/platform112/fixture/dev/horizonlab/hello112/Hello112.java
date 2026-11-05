package dev.horizonlab.hello112;

import net.minecraft.block.Block;
import net.minecraft.block.material.Material;
import net.minecraft.block.properties.PropertyInteger;
import net.minecraft.block.state.BlockStateContainer;
import net.minecraft.block.state.IBlockState;
import net.minecraft.creativetab.CreativeTabs;
import net.minecraft.item.Item;
import net.minecraft.item.ItemBlock;
import net.minecraft.item.ItemStack;
import net.minecraft.util.NonNullList;
import net.minecraftforge.event.RegistryEvent;
import net.minecraftforge.fluids.Fluid;
import net.minecraftforge.fluids.FluidRegistry;
import net.minecraft.util.ResourceLocation;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.event.FMLPreInitializationEvent;
import net.minecraftforge.fml.common.eventhandler.SubscribeEvent;

/**
 * The 1.12 bridge's test mod (docs/java-bridge-112.md, phase 1): the shapes the report must
 * carry. {@code marble}: three stored states by metadata, its item with subtypes; {@code lamp}:
 * a plain block giving light; {@code gem}: an item with two subtypes and their own names;
 * {@code wand}: a damageable tool; {@code goo}: a Forge fluid with its block.
 */
@Mod(modid = Hello112.ID, name = "Hello 1.12", version = "1.0", acceptedMinecraftVersions = "[1.12.2]")
@Mod.EventBusSubscriber
public final class Hello112 {
    public static final String ID = "hello112";

    public static final PropertyInteger VARIANT = PropertyInteger.create("variant", 0, 2);

    public static final Block MARBLE = new Block(Material.ROCK) {
        {
            setDefaultState(blockState.getBaseState().withProperty(VARIANT, 0));
        }

        @Override
        protected BlockStateContainer createBlockState() {
            return new BlockStateContainer(this, VARIANT);
        }

        @Override
        public IBlockState getStateFromMeta(int meta) {
            return getDefaultState().withProperty(VARIANT, Math.min(meta, 2));
        }

        @Override
        public int getMetaFromState(IBlockState state) {
            return state.getValue(VARIANT);
        }

        @Override
        public int damageDropped(IBlockState state) {
            return state.getValue(VARIANT);
        }

        @Override
        public void getSubBlocks(CreativeTabs tab, NonNullList<ItemStack> items) {
            for (int i = 0; i <= 2; i++) items.add(new ItemStack(this, 1, i));
        }
    }.setRegistryName(ID, "marble").setTranslationKey(ID + ".marble").setHardness(1.5F).setCreativeTab(CreativeTabs.BUILDING_BLOCKS);

    public static final Block LAMP = new Block(Material.GLASS)
            .setRegistryName(ID, "lamp").setTranslationKey(ID + ".lamp").setLightLevel(1.0F).setCreativeTab(CreativeTabs.DECORATIONS);

    public static final Item GEM = new Item() {
        {
            setHasSubtypes(true);
        }

        @Override
        public String getTranslationKey(ItemStack stack) {
            return super.getTranslationKey() + (stack.getMetadata() == 1 ? ".blue" : ".red");
        }

        @Override
        public void getSubItems(CreativeTabs tab, NonNullList<ItemStack> items) {
            if (isInCreativeTab(tab)) {
                items.add(new ItemStack(this, 1, 0));
                items.add(new ItemStack(this, 1, 1));
            }
        }
    }.setRegistryName(ID, "gem").setTranslationKey(ID + ".gem").setCreativeTab(CreativeTabs.MATERIALS);

    public static final Item WAND = new Item().setMaxStackSize(1).setMaxDamage(64)
            .setRegistryName(ID, "wand").setTranslationKey(ID + ".wand").setCreativeTab(CreativeTabs.TOOLS);

    public static final Fluid GOO = new Fluid("hello112_goo", new ResourceLocation(ID, "blocks/goo_still"), new ResourceLocation(ID, "blocks/goo_flow"))
            .setViscosity(3000).setTemperature(320);

    @Mod.EventHandler
    public void preInit(FMLPreInitializationEvent event) {
        FluidRegistry.registerFluid(GOO);
    }

    @SubscribeEvent
    public static void blocks(RegistryEvent.Register<Block> event) {
        event.getRegistry().registerAll(MARBLE, LAMP);
    }

    @SubscribeEvent
    public static void items(RegistryEvent.Register<Item> event) {
        ItemBlock marble = new ItemBlock(MARBLE) {
            @Override
            public int getMetadata(int damage) {
                return damage;
            }

            @Override
            public String getTranslationKey(ItemStack stack) {
                return super.getTranslationKey() + "." + stack.getMetadata();
            }
        };
        marble.setHasSubtypes(true);
        event.getRegistry().registerAll(
                marble.setRegistryName(MARBLE.getRegistryName()),
                new ItemBlock(LAMP).setRegistryName(LAMP.getRegistryName()),
                GEM, WAND);
    }

    /** The item models by meta (client only). */
    @Mod.EventBusSubscriber(value = net.minecraftforge.fml.relauncher.Side.CLIENT, modid = ID)
    public static final class ClientModels {
        @SubscribeEvent
        public static void models(net.minecraftforge.client.event.ModelRegistryEvent event) {
            for (int i = 0; i <= 2; i++) model(Item.getItemFromBlock(MARBLE), i, "marble_" + i);
            model(Item.getItemFromBlock(LAMP), 0, "lamp");
            model(GEM, 0, "gem_0");
            model(GEM, 1, "gem_1");
            model(WAND, 0, "wand");
        }

        private static void model(Item item, int meta, String name) {
            net.minecraftforge.client.model.ModelLoader.setCustomModelResourceLocation(item, meta,
                    new net.minecraft.client.renderer.block.model.ModelResourceLocation(new ResourceLocation(ID, name), "inventory"));
        }
    }
}
