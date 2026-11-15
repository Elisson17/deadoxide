/** Oracle probe (tools/oracle.sh javachars): java.lang.Character.isDigit/isLowerCase/isUpperCase of every UTF-16
 * unit on the game's JRE, the classes Kahlua's Lua patterns use (%d %l %u %a %w). One line per unit with any of
 * them: hex unit, then the flags (1 digit, 2 lower, 4 upper). Compared with `pz-dump javachars`. */
public class JavaCharsProbe {
    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        for (int c = 0; c < 65536; c++) {
            char ch = (char) c;
            int f = (Character.isDigit(ch) ? 1 : 0) | (Character.isLowerCase(ch) ? 2 : 0) | (Character.isUpperCase(ch) ? 4 : 0);
            if (f != 0) {
                sb.append(Integer.toHexString(c)).append(' ').append(f).append('\n');
            }
        }
        System.out.print(sb);
    }
}
