import java.util.Random;

/** Oracle probe (tools/oracle.sh fdlibm): java.lang.Math on the game's JRE for the functions Kahlua's MathLib and
 * J2SEPlatform.pow call, and StrictMath.sin/cos (Math.sin/cos are HotSpot intrinsics there, not compared). Inputs come
 * from java.util.Random(42): special values, raw random bit patterns and uniform values at several scales. Printed:
 * `name x [y] result` as raw hex bits (NaN canonicalised) for the first 4000 inputs of each function, then a
 * 31-multiplier hash of the result bits over 200000 further inputs. Compared with `pz-dump fdlibm`. */
public class FdLibmProbe {
    static final String[] ONE = {"tan", "asin", "acos", "atan", "exp", "log", "log10", "sinh", "cosh", "tanh", "sin", "cos", "expm1", "scalb"};
    static final String[] TWO = {"pow", "atan2"};
    static final double[] SPECIAL = {0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, -2.0, Double.POSITIVE_INFINITY,
        Double.NEGATIVE_INFINITY, Double.NaN, Double.MIN_VALUE, -Double.MIN_VALUE, Double.MAX_VALUE, -Double.MAX_VALUE,
        Double.MIN_NORMAL, Math.PI, Math.PI / 2, Math.PI / 4, -Math.PI / 2, 3.0, 10.0, 0.1, 709.782712893384,
        -745.1332191019411, 710.4758600739439, 22.0, 1e-300, 1e300, 1e22, 1e-8, 0.9999995, 1.0000009536743162,
        2.1474856959999995E9, 1e-320, 100.0, -100.0, 1e6, 7.0, -3.5};

    static long bits(double d) {
        return Double.doubleToLongBits(d);
    }

    static double input(Random r, int i) {
        if (i < SPECIAL.length) {
            return SPECIAL[i];
        }
        switch (i % 6) {
            case 0: return Double.longBitsToDouble(r.nextLong());
            case 1: return r.nextDouble() * 2 - 1;
            case 2: return (r.nextDouble() * 2 - 1) * 10;
            case 3: return (r.nextDouble() * 2 - 1) * 1000;
            case 4: return (r.nextDouble() * 2 - 1) * 1e6;
            default: return (r.nextDouble() * 2 - 1) * Math.scalb(1.0, r.nextInt(2100) - 1075);
        }
    }

    static double one(String f, double x, int n) {
        switch (f) {
            case "tan": return Math.tan(x);
            case "asin": return Math.asin(x);
            case "acos": return Math.acos(x);
            case "atan": return Math.atan(x);
            case "exp": return Math.exp(x);
            case "log": return Math.log(x);
            case "log10": return Math.log10(x);
            case "sinh": return Math.sinh(x);
            case "cosh": return Math.cosh(x);
            case "tanh": return Math.tanh(x);
            case "sin": return StrictMath.sin(x);
            case "cos": return StrictMath.cos(x);
            case "expm1": return Math.expm1(x);
            default: return Math.scalb(x, n);
        }
    }

    static double two(String f, double x, double y) {
        return f.equals("pow") ? Math.pow(x, y) : Math.atan2(x, y);
    }

    public static void main(String[] args) {
        StringBuilder sb = new StringBuilder();
        Random r = new Random(42);
        for (String f : ONE) {
            long h = 0;
            for (int i = 0; i < 204000; i++) {
                double x = input(r, i);
                int n = r.nextInt(4300) - 2150;
                long b = bits(one(f, x, n));
                if (i < 4000) {
                    sb.append(f).append(' ').append(Long.toHexString(bits(x)));
                    if (f.equals("scalb")) {
                        sb.append(' ').append(n);
                    }
                    sb.append(' ').append(Long.toHexString(b)).append('\n');
                } else {
                    h = h * 31 + b;
                }
            }
            sb.append(f).append(" hash ").append(Long.toHexString(h)).append('\n');
        }
        for (String f : TWO) {
            long h = 0;
            for (int i = 0; i < 204000; i++) {
                double x = input(r, i % 1600);
                double y = input(r, i / 40 % 40 + (i >= 1600 ? 40 : 0));
                if (f.equals("pow") && i % 3 == 0) {
                    y = Math.rint(y * 4) / 4;
                }
                long b = bits(two(f, x, y));
                if (i < 4000) {
                    sb.append(f).append(' ').append(Long.toHexString(bits(x))).append(' ')
                        .append(Long.toHexString(bits(y))).append(' ').append(Long.toHexString(b)).append('\n');
                } else {
                    h = h * 31 + b;
                }
            }
            sb.append(f).append(" hash ").append(Long.toHexString(h)).append('\n');
        }
        System.out.print(sb);
    }
}
