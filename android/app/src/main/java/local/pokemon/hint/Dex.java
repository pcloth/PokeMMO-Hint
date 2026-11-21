package local.pokemon.hint;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.*;
import java.util.regex.Pattern;

final class Dex {
    enum Language { ZH, EN, JA }
    record Species(int id, String[] names, int[] types) {
        String name(Language language) { return names[language.ordinal()]; }
    }
    private static final String[] KEYS = ("normal fighting flying poison ground rock bug ghost "
            + "steel fire water grass electric psychic ice dragon dark").split(" ");
    private static final String[] TYPES = "普通 格斗 飞行 毒 地面 岩石 虫 幽灵 钢 火 水 草 电 超能力 冰 龙 恶".split(" ");
    private static final Pattern NUMBER = Pattern.compile("(?<![a-z0-9])(?:no\\s*\\.?|#)\\s*([0-9]{1,3})(?![a-z0-9])");
    private static final Pattern LEVEL = Pattern.compile("[\\s　]*(?:lv\\.?|level)\\s*\\d+.*$");
    private final Map<Integer, Species> species = new TreeMap<>();
    private final Map<String, Set<Species>> names = new HashMap<>();
    private final int[][] chart = new int[17][17]; // units of 1/4

    Dex(InputStream base, InputStream translated, InputStream chartStream) throws IOException {
        try (BufferedReader data = reader(base); BufferedReader aliases = reader(translated);
             BufferedReader rules = reader(chartStream)) {
            Map<Integer, String[]> translations = new HashMap<>();
            for (String row; (row = aliases.readLine()) != null;) {
                if (row.startsWith("#") || row.trim().isEmpty()) continue;
                String[] fields = row.split("\t");
                translations.put(Integer.parseInt(fields[0]), fields);
            }
            for (String row; (row = data.readLine()) != null;) {
                if (row.startsWith("#") || row.trim().isEmpty()) continue;
                String[] fields = row.split("\t");
                int id = Integer.parseInt(fields[0]);
                String[] other = translations.get(id);
                int[] types = new int[fields.length - 2];
                for (int i = 0; i < types.length; i++) {
                    types[i] = Arrays.asList(KEYS).indexOf(fields[i + 2]);
                    if (types[i] < 0) throw new IOException("未知属性");
                }
                Species entry = new Species(id, new String[]{fields[1], other[1], other[2]}, types);
                species.put(id, entry);
                for (String name : entry.names()) {
                    names.computeIfAbsent(nameKey(name), key -> new HashSet<>()).add(entry);
                }
            }
            for (int[] row : chart) Arrays.fill(row, 4);
            for (int value : new int[]{8, 2, 0}) {
                for (int attack = 0; attack < 17; attack++) {
                    String line = rules.readLine();
                    if (line == null) throw new IOException("属性表不完整");
                    if (line.isEmpty()) continue;
                    for (String defender : line.split(",")) chart[attack][Integer.parseInt(defender)] = value;
                }
            }
        }
        if (species.size() != 649) throw new IOException("物种表不完整");
    }

    private static BufferedReader reader(InputStream stream) {
        return new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8));
    }

    private static String normalized(String text) {
        return Normalizer.normalize(text, Normalizer.Form.NFKC).toLowerCase(Locale.ROOT).trim();
    }

    private static String nameKey(String text) {
        return normalized(text).replaceAll("[\\p{Z}\\s.'’·-]", "");
    }

    Species lookup(String input, boolean manual) {
        String text = normalized(input);
        if (manual && text.matches("[0-9]{1,3}")) return species.get(Integer.parseInt(text));
        var match = NUMBER.matcher(text);
        Integer id = null;
        while (match.find()) {
            int next = Integer.parseInt(match.group(1));
            if (id != null && id != next) return null;
            id = next;
        }
        String remainder = NUMBER.matcher(text).replaceAll(" ").trim();
        remainder = LEVEL.matcher(remainder).replaceFirst("").trim();
        Set<Species> matches = nameMatches(remainder);
        // Gender markers that follow the level are already removed above.
        if (matches == null && (remainder.endsWith("♀") || remainder.endsWith("♂"))) {
            matches = nameMatches(remainder.substring(0, remainder.length() - 1));
        }
        if (matches != null && matches.size() != 1) return null;
        Species named = matches == null ? null : matches.iterator().next();
        if (id == null) return named;
        return named == null || named.id() == id ? species.get(id) : null;
    }

    private Set<Species> nameMatches(String name) { return names.get(nameKey(name)); }

    int multiplier(int attack, Species entry) {
        int factor = 4;
        for (int type : entry.types()) factor = factor * chart[attack][type] / 4;
        return factor;
    }

    String label(Species entry) {
        for (int factor : new int[]{16, 8}) {
            for (int attack = 0; attack < TYPES.length; attack++) {
                if (multiplier(attack, entry) == factor) return TYPES[attack] + suffix(factor);
            }
        }
        return "无弱点";
    }

    String title(Species entry, Language lang) {
        List<String> types = new ArrayList<>();
        for (int type : entry.types()) types.add(TYPES[type]);
        return String.format(Locale.ROOT, "#%03d  %s · %s", entry.id(), entry.name(lang), String.join(" / ", types));
    }

    String[] details(Species entry) {
        List<List<String>> groups = Arrays.asList(new ArrayList<>(), new ArrayList<>(), new ArrayList<>(), new ArrayList<>());
        for (int factor : new int[]{16, 8, 4, 1, 2, 0}) {
            int group = factor > 4 ? 0 : factor == 4 ? 1 : factor > 0 ? 2 : 3;
            for (int attack = 0; attack < TYPES.length; attack++) {
                if (multiplier(attack, entry) == factor) groups.get(group).add(TYPES[attack] + suffix(factor));
            }
        }
        return groups.stream().map(group -> group.isEmpty() ? "无" : String.join("、", group)).toArray(String[]::new);
    }

    private static String suffix(int factor) {
        return switch (factor) { case 16 -> "×4"; case 8 -> "×2"; case 2 -> "×½"; case 1 -> "×¼"; default -> ""; };
    }
}
