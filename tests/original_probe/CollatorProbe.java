import java.text.CollationElementIterator;
import java.text.Collator;
import java.text.RuleBasedCollator;
import java.util.Locale;

/** Measurement (no Rust side): the collation elements `Collator.getInstance(Locale.ENGLISH)` gives each printable ASCII
 * character, the table behind `java_collator::ENGLISH_ASCII` (what ActionState.load sorts file names with). Prints one
 * Rust array entry per character, 0x20..=0x7e, with the order ints of `getCollationElementIterator` (a character with
 * more or fewer than one element, or a pair of characters that is not the concatenation of its characters' elements,
 * is reported instead, since the port assumes neither happens in this range); then strength and decomposition. */
public class CollatorProbe {
    static int[] orders(RuleBasedCollator c, String s) {
        CollationElementIterator it = c.getCollationElementIterator(s);
        int[] r = new int[s.length() * 4];
        int n = 0;
        for (int o = it.next(); o != CollationElementIterator.NULLORDER; o = it.next()) r[n++] = o;
        return java.util.Arrays.copyOf(r, n);
    }

    public static void main(String[] a) {
        RuleBasedCollator c = (RuleBasedCollator) Collator.getInstance(Locale.ENGLISH);
        for (char ch = 0x20; ch <= 0x7e; ch++) {
            int[] o = orders(c, String.valueOf(ch));
            if (o.length != 1) System.out.println("// MULTI " + (int) ch + " " + o.length);
            else System.out.println("    0x" + Integer.toHexString(o[0]) + ", // " + (int) ch);
        }
        for (char x = 0x20; x <= 0x7e; x++)
            for (char y = 0x20; y <= 0x7e; y++) {
                int[] o = orders(c, "" + x + y);
                int[] ox = orders(c, String.valueOf(x)), oy = orders(c, String.valueOf(y));
                if (o.length != 2 || ox.length != 1 || oy.length != 1 || o[0] != ox[0] || o[1] != oy[0])
                    System.out.println("// CONTRACTION " + (int) x + " " + (int) y);
            }
        System.out.println("// strength " + c.getStrength() + " decomposition " + c.getDecomposition());
    }
}
