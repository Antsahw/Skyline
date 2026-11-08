package dev.horizonlab.bridge.r120;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Reads JSON into Maps, Lists, Strings, Doubles, Booleans and nulls: mods' fabric.mod.json and
 * the dependency override files, before Fabric Loader (and its Gson) is up. Lenient like
 * Fabric's own reader where mods rely on it: comments, a trailing comma, raw control characters
 * in strings.
 */
final class MiniJson {
    private final String s;
    private int i;

    private MiniJson(String s) {
        this.s = s;
    }

    static Object parse(String text) {
        MiniJson p = new MiniJson(text.startsWith("﻿") ? text.substring(1) : text);
        Object v = p.value();
        p.space();
        if (p.i != p.s.length()) throw p.error("text after the value");
        return v;
    }

    private IllegalArgumentException error(String what) {
        return new IllegalArgumentException("JSON: " + what + " at " + i);
    }

    private void space() {
        while (i < s.length()) {
            char c = s.charAt(i);
            if (Character.isWhitespace(c)) {
                i++;
            } else if (s.startsWith("//", i)) {
                while (i < s.length() && s.charAt(i) != '\n') i++;
            } else if (s.startsWith("/*", i)) {
                int end = s.indexOf("*/", i + 2);
                i = end < 0 ? s.length() : end + 2;
            } else {
                return;
            }
        }
    }

    private Object value() {
        space();
        if (i >= s.length()) throw error("end of text");
        char c = s.charAt(i);
        if (c == '{') return object();
        if (c == '[') return array();
        if (c == '"') return string();
        if (s.startsWith("true", i)) {
            i += 4;
            return Boolean.TRUE;
        }
        if (s.startsWith("false", i)) {
            i += 5;
            return Boolean.FALSE;
        }
        if (s.startsWith("null", i)) {
            i += 4;
            return null;
        }
        int start = i;
        while (i < s.length() && "+-0123456789.eE".indexOf(s.charAt(i)) >= 0) i++;
        if (start == i) throw error("unexpected '" + c + "'");
        return Double.parseDouble(s.substring(start, i));
    }

    private Map<String, Object> object() {
        Map<String, Object> out = new LinkedHashMap<>();
        i++;
        while (true) {
            space();
            if (i < s.length() && s.charAt(i) == '}') {
                i++;
                return out;
            }
            String key = string();
            space();
            if (i >= s.length() || s.charAt(i) != ':') throw error("':' expected");
            i++;
            out.put(key, value());
            space();
            if (i < s.length() && s.charAt(i) == ',') {
                i++;
            } else if (i < s.length() && s.charAt(i) == '}') {
                i++;
                return out;
            } else {
                throw error("',' or '}' expected");
            }
        }
    }

    private List<Object> array() {
        List<Object> out = new ArrayList<>();
        i++;
        while (true) {
            space();
            if (i < s.length() && s.charAt(i) == ']') {
                i++;
                return out;
            }
            out.add(value());
            space();
            if (i < s.length() && s.charAt(i) == ',') {
                i++;
            } else if (i < s.length() && s.charAt(i) == ']') {
                i++;
                return out;
            } else {
                throw error("',' or ']' expected");
            }
        }
    }

    private String string() {
        space();
        if (i >= s.length() || s.charAt(i) != '"') throw error("string expected");
        i++;
        StringBuilder out = new StringBuilder();
        while (i < s.length()) {
            char c = s.charAt(i++);
            if (c == '"') return out.toString();
            if (c != '\\') {
                out.append(c);
                continue;
            }
            if (i >= s.length()) break;
            char e = s.charAt(i++);
            switch (e) {
                case 'n' -> out.append('\n');
                case 't' -> out.append('\t');
                case 'r' -> out.append('\r');
                case 'b' -> out.append('\b');
                case 'f' -> out.append('\f');
                case 'u' -> {
                    out.append((char) Integer.parseInt(s.substring(i, i + 4), 16));
                    i += 4;
                }
                default -> out.append(e);
            }
        }
        throw error("unterminated string");
    }
}
