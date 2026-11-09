package dev.horizonlab.platform120;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;

/**
 * Mojang's names of 1.20.1's classes at run time, where Fabric runs them under intermediary names
 * ({@code net.minecraft.class_2248}): Horizon's content tables and the carry-over's templates
 * name vanilla classes as Mojang does. The table ({@code horizon/classes-1.20.1.tsv}: intermediary,
 * tab, Mojang's name) is made by build120.sh from Mojang's mappings into the platform jar, which
 * stays in the player's cache.
 */
final class Names {
    private Names() {}

    private static Map<String, String> table;

    /** Mojang's name of a class; a mod's class (or one the table lacks) as it is. */
    static String mojang(Class<?> type) {
        String name = type.getName();
        if (!name.startsWith("net.minecraft.")) return name;
        return load().getOrDefault(name, name);
    }

    private static synchronized Map<String, String> load() {
        if (table != null) return table;
        table = new HashMap<>();
        try (InputStream in = Names.class.getResourceAsStream("/horizon/classes-1.20.1.tsv")) {
            if (in != null) {
                BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
                for (String line; (line = r.readLine()) != null; ) {
                    int tab = line.indexOf('\t');
                    if (tab > 0) table.put(line.substring(0, tab), line.substring(tab + 1));
                }
            }
        } catch (IOException e) {
            // No table: names stay intermediary.
        }
        return table;
    }
}
