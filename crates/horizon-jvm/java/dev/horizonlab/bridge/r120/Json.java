package dev.horizonlab.bridge.r120;

import java.util.Collection;
import java.util.Map;

/** A small JSON writer for the report: JDK types only (no Gson on the system class path). */
final class Json {
    private Json() {}

    static String write(Object value) {
        StringBuilder out = new StringBuilder();
        write(out, value);
        return out.toString();
    }

    static void write(StringBuilder out, Object v) {
        if (v == null) {
            out.append("null");
        } else if (v instanceof String s) {
            string(out, s);
        } else if (v instanceof Number || v instanceof Boolean) {
            out.append(v);
        } else if (v instanceof Map<?, ?> m) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> e : m.entrySet()) {
                if (!first) out.append(',');
                first = false;
                string(out, String.valueOf(e.getKey()));
                out.append(':');
                write(out, e.getValue());
            }
            out.append('}');
        } else if (v instanceof Collection<?> c) {
            out.append('[');
            boolean first = true;
            for (Object o : c) {
                if (!first) out.append(',');
                first = false;
                write(out, o);
            }
            out.append(']');
        } else {
            string(out, String.valueOf(v));
        }
    }

    private static void string(StringBuilder out, String s) {
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
