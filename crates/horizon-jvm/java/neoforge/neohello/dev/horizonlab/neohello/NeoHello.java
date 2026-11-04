package dev.horizonlab.neohello;

import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.ModContainer;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.server.ServerStartedEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;
import net.neoforged.neoforge.registries.DeferredBlock;
import net.neoforged.neoforge.registries.DeferredItem;
import net.neoforged.neoforge.registries.DeferredRegister;

/**
 * The bridge's NeoForge fixture: one block and its item (deferred registers on the mod bus),
 * common setup, and two game-bus listeners. Each step logs a "[neohello]" line a test can grep.
 */
@Mod("neohello")
public final class NeoHello {
    public static final DeferredRegister.Blocks BLOCKS = DeferredRegister.createBlocks("neohello");
    public static final DeferredRegister.Items ITEMS = DeferredRegister.createItems("neohello");
    public static final DeferredBlock<Block> HELLO_BLOCK =
            BLOCKS.registerSimpleBlock("hello_block", p -> p.strength(1.5f));
    public static final DeferredItem<BlockItem> HELLO_BLOCK_ITEM = ITEMS.registerSimpleBlockItem(HELLO_BLOCK);

    private static int ticks;

    public NeoHello(IEventBus modBus, ModContainer container) {
        System.out.println("[neohello] constructed by " + container.getModId());
        // -Dneohello.stack=true: who constructs mods (where a host takes over from the loader).
        if (Boolean.getBoolean("neohello.stack")) dumpStacks("construction");
        BLOCKS.register(modBus);
        ITEMS.register(modBus);
        modBus.addListener(FMLCommonSetupEvent.class, e -> {
            System.out.println("[neohello] common setup; block registered as " + HELLO_BLOCK.getId()
                    + " -> " + HELLO_BLOCK.get());
            if (Boolean.getBoolean("neohello.stack")) dumpStacks("common setup");
        });
        NeoForge.EVENT_BUS.addListener(ServerStartedEvent.class, e ->
                System.out.println("[neohello] server started: " + e.getServer().getServerVersion()));
        NeoForge.EVENT_BUS.addListener(ServerTickEvent.Post.class, e -> {
            if (++ticks % 100 == 0) System.out.println("[neohello] server tick " + ticks);
        });
    }

    /** The loader's own threads at a loading stage: who drives it (where a host takes over). */
    private static void dumpStacks(String stage) {
        Thread.getAllStackTraces().forEach((thread, frames) -> {
            String name = thread.getName();
            if (!name.equals("main") && !name.equals("Render thread")) return;
            System.out.println("[neohello] " + name + " during " + stage + ":");
            for (StackTraceElement frame : frames) System.out.println("\tat " + frame);
        });
    }
}
