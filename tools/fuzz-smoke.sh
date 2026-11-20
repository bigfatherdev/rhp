#!/bin/bash
# tools/fuzz-smoke.sh — fuzz mutacional rapido do RHP (auditoria full-system).
#
# Gera programas mutados a partir de sementes validas e procura abort
# (SIGABRT/SIGSEGV), hang e panics. Time-boxed por padrao (1000 casos).
#
# Uso: ./tools/fuzz-smoke.sh [N] [seed] [binario-rhp]
#   N padrao 300; timeout 5 s/caso. Saida != 0 se houver suspeita.
set -u
N="${1:-300}"
SEED="${2:-42}"
BIN="${3:-./target/debug/rhp}"
test -x "$BIN" || { echo "binario nao encontrado: $BIN (rode cargo build antes)" >&2; exit 2; }
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
python3 - "$N" "$SEED" "$TMP" "$BIN" <<'PYEOF'
import random, subprocess, sys, os
n, seed, tmp, rhp = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3], sys.argv[4]
rng = random.Random(seed)
SEEDS = [
    'echo 1 + 2 * 3;\n',
    'fn f(a, b) {\n  if a > b {\n    return a;\n  }\n  return b;\n}\necho f(1, 2);\n',
    'echo true && false || true;\n',
    'echo contains([1, 2], 2);\n',
    'try {\n  throw "e";\n} catch e {\n  echo e;\n} finally {\n  echo "f";\n}\n',
    'echo match 1 { 1 => "um", default => "outro" };\n',
    'let f = fn (x) => x * 2;\necho map([1, 2], f);\n',
    'echo json_parse("{\\"a\\": [1, 2]}");\n',
    'switch 2 {\n  case 1 {\n    echo "um";\n  }\n  default {\n    echo "d";\n  }\n}\n',
]
TOKENS = ['!', '(', ')', '[', ']', '{', '}', '"', "'", '?', ':', ';', ',', '.', '+', '-', '*', '/', '%', '=', '<', '>', '&', '|', '^', '~', 'echo', 'let', 'fn', 'if', 'else', 'while', 'for', 'return', 'try', 'catch', 'finally', 'throw', 'true', 'null', '0', 'x', ' ', '\n', '&&', '||']
def mutate(s):
    for _ in range(rng.randint(1, 6)):
        op = rng.random()
        if not s:
            s = rng.choice(TOKENS)
            continue
        if op < 0.35:
            i = rng.randrange(len(s)); s = s[:i] + rng.choice(TOKENS) + s[i+1:]
        elif op < 0.6:
            i = rng.randrange(len(s) + 1); s = s[:i] + rng.choice(TOKENS) + s[i:]
        elif op < 0.8:
            i = rng.randrange(len(s)); s = s[:i] + s[i+rng.randint(1, 8):]
        else:
            i = rng.randrange(len(s)); j = min(len(s), i + rng.randint(1, 20)); k = rng.randrange(len(s) + 1)
            s = s[:k] + s[i:j] + s[k:]
    return s
bad = 0
for i in range(n):
    src = mutate(rng.choice(SEEDS))
    p = os.path.join(tmp, f"f{i}.rhp")
    open(p, 'w').write(src)
    try:
        r = subprocess.run([rhp, "run", p], capture_output=True, timeout=5, text=True, errors="replace")
        out = (r.stdout + r.stderr).lower()
        if r.returncode in (-6, -11, 134, 139) or "panicked" in out or "stack overflow" in out or "has overflowed" in out:
            print(f"CRASH f{i}: rc={r.returncode} :: {src[:100]!r}")
            bad += 1
    except subprocess.TimeoutExpired:
        print(f"HANG f{i}: {src[:100]!r}")
        bad += 1
print(f"done: {n} casos, {bad} suspeitos")
sys.exit(1 if bad else 0)
PYEOF
