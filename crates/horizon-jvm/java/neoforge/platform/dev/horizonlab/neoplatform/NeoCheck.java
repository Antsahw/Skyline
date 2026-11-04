package dev.horizonlab.neoplatform;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;

/** What the standalone test prints: the loaded mods and their registered content. */
public final class NeoCheck {
    private NeoCheck() {}

    public static List<String> report() {
        List<String> out = new ArrayList<>();
        for (String id : new String[] {"neohello:hello_block", "minecraft:stone"}) {
            Identifier key = Identifier.parse(id);
            out.add("block " + id + " registered: " + BuiltInRegistries.BLOCK.containsKey(key)
                    + ", item: " + BuiltInRegistries.ITEM.containsKey(key));
        }
        out.add("blocks " + BuiltInRegistries.BLOCK.size() + ", items " + BuiltInRegistries.ITEM.size());
        out.add("game loader " + NeoCheck.class.getClassLoader());
        return out;
    }
}
