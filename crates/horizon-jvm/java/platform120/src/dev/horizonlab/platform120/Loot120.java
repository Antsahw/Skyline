package dev.horizonlab.platform120;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.Reader;
import java.util.Map;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.level.storage.loot.LootDataId;
import net.minecraft.world.level.storage.loot.LootDataType;
import net.minecraft.world.level.storage.loot.LootTable;

/**
 * What the 1.20.1 mods add to loot tables in code (Fabric's {@code LootTableEvents.MODIFY}:
 * Farmer's Delight's straw from grass, a mod's items in vanilla's chests). Fabric applies those as
 * a server loads its data; the mirror server's data loaded the same way, so each table's pools
 * there are its file's and then the mods'. The pools a table has beyond its file's are what the
 * mods added, written as data for Horizon to add when it loads the table.
 */
final class Loot120 {
    private Loot120() {}

    /** {table id: [the pools the mods added, as 1.20.1's JSON]}, for every table they grew. */
    static String added() {
        var data = Mirror120.data();
        var manager = data.getLootData();
        var gson = LootDataType.TABLE.parser();
        JsonObject out = new JsonObject();
        Map<ResourceLocation, net.minecraft.server.packs.resources.Resource> files =
                Mirror120.dataPacks().listResources("loot_tables", rl -> rl.getPath().endsWith(".json"));
        for (var file : files.entrySet()) {
            String path = file.getKey().getPath();
            var id = new ResourceLocation(file.getKey().getNamespace(),
                    path.substring("loot_tables/".length(), path.length() - ".json".length()));
            LootTable loaded = manager.getElement(new LootDataId<>(LootDataType.TABLE, id));
            if (loaded == null) continue;
            JsonArray own;
            try (Reader reader = file.getValue().openAsReader()) {
                JsonElement raw = JsonParser.parseReader(reader);
                own = pools(gson.toJsonTree(gson.fromJson(raw, LootTable.class)));
            } catch (Exception e) {
                continue;
            }
            JsonArray now = pools(gson.toJsonTree(loaded));
            if (now.size() <= own.size()) continue;
            JsonArray extra = new JsonArray();
            for (int i = own.size(); i < now.size(); i++) extra.add(now.get(i));
            out.add(id.toString(), extra);
        }
        return out.toString();
    }

    private static JsonArray pools(JsonElement table) {
        if (table instanceof JsonObject o && o.get("pools") instanceof JsonArray pools) return pools;
        return new JsonArray();
    }
}
