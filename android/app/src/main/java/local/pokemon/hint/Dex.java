package local.pokemon.hint;

import android.content.Context;
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
    record Form(String key, String[] names, int[] types) {
        String name(Language language) { return names[language.ordinal()]; }
    }
    enum Tab {
        LEGACY("PokeMMO"), MODERN("现代"), TERA("太晶化"), DYNAMAX("极巨化");

        final String label;
        Tab(String label) { this.label = label; }
    }
    record Typing(Tab tab, int form, int tera, boolean gigantamax) {
        static final Typing BASE = new Typing(null, -1, -1, false);
        static final int STELLAR = 18;

        Typing withTab(Tab selected) { return new Typing(selected, form, tera, gigantamax); }
    }
    record Card(Dex rules, Species base, Species entry, Tab tab, Typing typing, boolean dynamax) {
        boolean supports(Tab other) {
            return switch (other) {
                case LEGACY -> base.id() <= LEGACY_SPECIES;
                case DYNAMAX -> dynamax;
                default -> true;
            };
        }
    }
    record Catalogs(Dex legacy, Dex modern) {
        Dex ocr(boolean modernMode) { return modernMode ? modern : legacy; }

        Card card(int id, boolean modernMode, Typing typing) {
            boolean legacyAvailable = legacy.species.containsKey(id);
            boolean dynamax = modern.dynamaxForms.containsKey(id);
            Tab tab = typing.tab();
            if (tab == null || tab == Tab.LEGACY && !legacyAvailable || tab == Tab.DYNAMAX && !dynamax) {
                tab = modernMode || !legacyAvailable ? Tab.MODERN : Tab.LEGACY;
            }
            Dex rules = tab == Tab.LEGACY ? legacy : modern;
            Species base = rules.species.get(id);
            return new Card(rules, base, rules.effective(base, tab, typing), tab, typing, dynamax);
        }
    }
    static final int MAX_SPECIES = 1025;
    private static final int LEGACY_SPECIES = 649;
    // Species with -gmax forms in PokéAPI pokemon.csv, as in the Windows chart.
    private static final Set<Integer> GIGANTAMAX = new HashSet<>(Arrays.asList(
            3, 6, 9, 12, 25, 52, 68, 94, 99, 131, 133, 143, 569, 809, 812, 815,
            818, 823, 826, 834, 839, 841, 842, 844, 849, 851, 858, 861, 869, 879, 884, 892));
    private static final String[] KEYS = ("normal fighting flying poison ground rock bug ghost "
            + "steel fire water grass electric psychic ice dragon dark").split(" ");
    private static final String[] TYPES = "普通 格斗 飞行 毒 地面 岩石 虫 幽灵 钢 火 水 草 电 超能力 冰 龙 恶 妖精".split(" ");
    private static final Pattern NUMBER = Pattern.compile("(?<![a-z0-9])(?:no\\s*\\.?|#)\\s*([0-9]{1,4})(?![a-z0-9])");
    private static final Pattern LEVEL = Pattern.compile("[\\s　]*(?:lv\\.?|level)\\s*\\d+.*$");
    private final Map<Integer, Species> species = new TreeMap<>();
    private final Map<String, Set<Species>> names = new HashMap<>();
    private final Map<Integer, List<Form>> forms = new HashMap<>();
    private final Map<Integer, Set<String>> dynamaxForms = new HashMap<>();
    private int[][] chart = new int[17][17]; // units of 1/4

    private Dex() {}

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
        if (species.size() != LEGACY_SPECIES) throw new IOException("物种表不完整");
        indexNames();
    }

    static Catalogs load(Context context) throws IOException {
        var assets = context.getAssets();
        Dex legacy = new Dex(assets.open("species.tsv"), assets.open("species_names.tsv"), assets.open("type_chart.txt"));
        Dex modern = new Dex();
        modern.species.putAll(legacy.species);
        try (var variants = reader(assets.open("modern_forms.tsv"));
             var entries = reader(assets.open("modern_species.tsv"));
             var rules = reader(assets.open("modern_chart.tsv"));
             var dynamax = reader(assets.open("dynamax.tsv"))) {
            modern.loadModern(entries, variants, rules);
            modern.loadDynamax(dynamax);
        }
        return new Catalogs(legacy, modern);
    }

    void loadModern(BufferedReader entries, BufferedReader variants, BufferedReader rules) throws IOException {
        for (String line; (line = entries.readLine()) != null;) {
            if (line.startsWith("#") || line.isEmpty()) continue;
            String[] row = line.split("\t");
            int id = Integer.parseInt(row[0]);
            Species existing = species.get(id);
            String[] labels = existing == null ? new String[]{row[2], row[3], row[4]} : existing.names();
            species.put(id, new Species(id, labels, parseTypes(row[1])));
        }
        if (species.size() != MAX_SPECIES) throw new IOException("现代物种表不完整");
        for (String line; (line = variants.readLine()) != null;) {
            if (line.startsWith("#") || line.isEmpty()) continue;
            String[] row = line.split("\t");
            if (row[1].endsWith("-gmax")) continue;
            forms.computeIfAbsent(Integer.parseInt(row[0]), id -> new ArrayList<>())
                    .add(new Form(row[1], new String[]{row[3], row[4], row[5]}, parseTypes(row[2])));
        }
        chart = new int[TYPES.length][TYPES.length];
        for (int attack = 0; attack < chart.length; attack++) {
            String line = rules.readLine();
            if (line == null) throw new IOException("现代属性表不完整");
            String[] row = line.split("\t");
            if (row.length != chart.length) throw new IOException("现代属性表不完整");
            for (int defense = 0; defense < chart.length; defense++) {
                chart[attack][defense] = Integer.parseInt(row[defense]) / 25;
            }
        }
        indexNames();
    }

    void loadDynamax(BufferedReader data) throws IOException {
        for (String line; (line = data.readLine()) != null;) {
            if (line.startsWith("#") || line.isEmpty()) continue;
            String[] row = line.split("\t");
            dynamaxForms.put(Integer.parseInt(row[0]), row.length == 1
                    ? Collections.emptySet() : new HashSet<>(Arrays.asList(row[1].split(","))));
        }
    }

    private void indexNames() {
        names.clear();
        for (Species entry : species.values()) {
            for (String name : entry.names()) names.computeIfAbsent(nameKey(name), key -> new HashSet<>()).add(entry);
        }
    }

    private static int[] parseTypes(String values) {
        return Arrays.stream(values.split(",")).mapToInt(Integer::parseInt).toArray();
    }

    List<Form> forms(Species entry) {
        List<Form> found = forms.get(entry.id());
        return found == null ? Collections.emptyList() : found;
    }

    private Form selectedForm(Species entry, int index) {
        List<Form> available = forms(entry);
        return index >= 0 && index < available.size() ? available.get(index) : null;
    }

    boolean dynamaxForm(Species entry, Form form) {
        return dynamaxForms.getOrDefault(entry.id(), Collections.emptySet()).contains(form.key());
    }

    boolean canGigantamax(Species entry, int index) {
        Form form = selectedForm(entry, index);
        return GIGANTAMAX.contains(entry.id()) && (form == null || !dynamaxForm(entry, form)
                || form.key().equals("urshifu-rapid-strike") || form.key().equals("toxtricity-low-key"));
    }

    int fixedTera(Species entry, int index) {
        if (entry.id() == 1024) return Typing.STELLAR;
        if (entry.id() != 1017) return -1;
        Form form = selectedForm(entry, index);
        return form == null ? 11 : form.types()[1];
    }

    int activeTera(Species entry, Typing selection) {
        int fixed = fixedTera(entry, selection.form());
        return selection.tera() < 0 || fixed < 0 ? selection.tera() : fixed;
    }

    private Species effective(Species entry, Tab tab, Typing selection) {
        if (tab == Tab.LEGACY) return entry;
        Form form = selectedForm(entry, selection.form());
        if (tab == Tab.DYNAMAX && form != null && !dynamaxForm(entry, form)) form = null;
        int tera = tab == Tab.TERA ? activeTera(entry, selection) : -1;
        if (form == null && tera < 0 && tab != Tab.DYNAMAX) return entry;
        int[] types = tera >= 0 && tera < chart.length ? new int[]{tera} : form != null ? form.types() : entry.types();
        String[] labels = entry.names().clone();
        for (Language language : Language.values()) {
            int index = language.ordinal();
            if (form != null) labels[index] += " · " + form.name(language);
            if (tera >= 0) labels[index] += " · 太晶" + (tera == Typing.STELLAR ? "星晶" : typeName(tera));
            if (tab == Tab.DYNAMAX) labels[index] += selection.gigantamax()
                    && canGigantamax(entry, selection.form()) ? " · 超极巨化" : " · 极巨化";
        }
        return new Species(entry.id(), labels, types);
    }

    static BufferedReader reader(InputStream stream) {
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
        if (manual && text.matches("[0-9]{1,4}")) return species.get(Integer.parseInt(text));
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

    String label(Species entry, boolean all) {
        StringJoiner weaknesses = new StringJoiner("、").setEmptyValue("无弱点");
        for (int factor : new int[]{16, 8}) {
            for (int attack = 0; attack < chart.length; attack++) {
                if (multiplier(attack, entry) != factor) continue;
                String value = TYPES[attack] + suffix(factor);
                if (!all) return value;
                weaknesses.add(value);
            }
        }
        return weaknesses.toString();
    }

    static String typeName(int type) { return TYPES[type]; }

    String title(Species entry, Language lang) {
        List<String> types = new ArrayList<>();
        for (int type : entry.types()) types.add(TYPES[type]);
        return String.format(Locale.ROOT, "#%03d  %s · %s", entry.id(), entry.name(lang), String.join(" / ", types));
    }

    String[] details(Species entry) {
        List<List<String>> groups = Arrays.asList(new ArrayList<>(), new ArrayList<>(), new ArrayList<>(), new ArrayList<>());
        for (int factor : new int[]{16, 8, 4, 1, 2, 0}) {
            int group = factor > 4 ? 0 : factor == 4 ? 1 : factor > 0 ? 2 : 3;
            for (int attack = 0; attack < chart.length; attack++) {
                if (multiplier(attack, entry) == factor) groups.get(group).add(TYPES[attack] + suffix(factor));
            }
        }
        return groups.stream().map(group -> group.isEmpty() ? "无" : String.join("、", group)).toArray(String[]::new);
    }

    private static String suffix(int factor) {
        return switch (factor) { case 16 -> "×4"; case 8 -> "×2"; case 2 -> "×½"; case 1 -> "×¼"; default -> ""; };
    }
}
