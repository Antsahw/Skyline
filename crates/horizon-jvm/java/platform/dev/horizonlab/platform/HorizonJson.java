package dev.horizonlab.platform;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.util.Map;

/**
 * JSON written fast, for what goes to Horizon every tick (the entity snapshot): Gson's own
 * writer formats each float through Float.toString's shortest-decimal search, which was a third
 * of the snapshot's time. Floats that are whole ten-thousandths (the snapshot rounds its
 * matrices and poses so) are written by integer arithmetic; the rest as Gson would.
 */
final class HorizonJson {
    private HorizonJson() {}

    static String write(JsonElement element) {
        StringBuilder out = new StringBuilder(4096);
        write(element, out);
        return out.toString();
    }

    static void write(JsonElement element, StringBuilder out) {
        if (element == null || element.isJsonNull()) {
            out.append("null");
        } else if (element instanceof JsonObject object) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<String, JsonElement> e : object.entrySet()) {
                if (!first) out.append(',');
                first = false;
                string(e.getKey(), out);
                out.append(':');
                write(e.getValue(), out);
            }
            out.append('}');
        } else if (element instanceof JsonArray array) {
            out.append('[');
            for (int i = 0; i < array.size(); i++) {
                if (i > 0) out.append(',');
                write(array.get(i), out);
            }
            out.append(']');
        } else if (element instanceof JsonPrimitive primitive) {
            if (primitive.isString()) {
                string(primitive.getAsString(), out);
            } else if (primitive.isBoolean()) {
                out.append(primitive.getAsBoolean());
            } else {
                number(primitive.getAsNumber(), out);
            }
        }
    }

    private static void number(Number n, StringBuilder out) {
        if (n instanceof Integer || n instanceof Long || n instanceof Short || n instanceof Byte) {
            out.append(n.longValue());
            return;
        }
        if (n instanceof Float f) {
            float v = f;
            long scaled = Math.round(v * 10000.0);
            if (Float.isFinite(v) && Math.abs(scaled) < 1L << 50 && Math.round(scaled / 10000.0F * 10000.0) == scaled
                    && scaled / 10000.0F == v) {
                if (scaled < 0) {
                    out.append('-');
                    scaled = -scaled;
                }
                out.append(scaled / 10000);
                long frac = scaled % 10000;
                if (frac != 0) {
                    out.append('.');
                    int digits = 4;
                    while (frac % 10 == 0) {
                        frac /= 10;
                        digits--;
                    }
                    String s = Long.toString(frac);
                    for (int i = s.length(); i < digits; i++) out.append('0');
                    out.append(s);
                }
                return;
            }
        }
        double d = n.doubleValue();
        if (!Double.isFinite(d)) {
            out.append('0');
        } else if (n instanceof Float) {
            out.append(n.floatValue());
        } else {
            out.append(d);
        }
    }

    private static void string(String s, StringBuilder out) {
        out.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) out.append(String.format("\\u%04x", (int) c));
                    else out.append(c);
                }
            }
        }
        out.append('"');
    }
}
