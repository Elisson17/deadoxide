import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import zombie.scripting.ScriptParser;

// The ORIGINAL zombie.scripting.ScriptParser over the cases of tests/script_parser_cases.txt (each case is the text
// after a line "@@case" up to the next one; the header before the first is skipped). Per case: stripComments, the
// parseTokens of the stripped text, and the tree parse() builds from the case text: every element in order, blocks
// with type and id (and their value/child counts), values with their string, getKey() and getValue(). Strings: '\\',
// tab, CR, LF and every UTF-16 unit outside 0x20..0x7e escaped (\\uXXXX); null as "null". No server needed.
// Args: <cases file>
public class ScriptParserProbe {
    static String esc(String s) {
        if (s == null) return "null";
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\') sb.append("\\\\");
            else if (c == '\t') sb.append("\\t");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c < 0x20 || c > 0x7e) sb.append(String.format("\\u%04x", (int) c));
            else sb.append(c);
        }
        return sb.toString();
    }

    static void tree(StringBuilder sb, ScriptParser.Block b, int depth) {
        sb.append("B\t").append(depth).append('\t').append(esc(b.type)).append('\t').append(esc(b.id))
            .append("\tvalues\t").append(b.values.size()).append("\tchildren\t").append(b.children.size()).append('\n');
        for (ScriptParser.BlockElement e : b.elements) {
            if (e.asBlock() != null) {
                tree(sb, e.asBlock(), depth + 1);
            } else {
                ScriptParser.Value v = e.asValue();
                sb.append("V\t").append(depth + 1).append('\t').append(esc(v.string)).append("\tkey\t").append(esc(v.getKey()))
                    .append("\tvalue\t").append(esc(v.getValue())).append('\n');
            }
        }
    }

    public static void main(String[] a) throws Exception {
        String text = new String(Files.readAllBytes(Paths.get(a[0])), StandardCharsets.UTF_8);
        String[] parts = text.split("(^|\n)@@case\n", -1);
        StringBuilder sb = new StringBuilder();
        for (int n = 1; n < parts.length; n++) {
            String c = parts[n];
            sb.append("case\t").append(n).append('\n');
            String stripped = ScriptParser.stripComments(c);
            sb.append("strip\t").append(esc(stripped)).append('\n');
            ArrayList<String> tokens = ScriptParser.parseTokens(stripped);
            for (String t : tokens) sb.append("token\t").append(esc(t)).append('\n');
            tree(sb, ScriptParser.parse(c), 0);
        }
        System.out.print(sb);
        System.out.flush();
        Runtime.getRuntime().halt(0);
    }
}
