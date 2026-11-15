#!/bin/bash
# tools/check-changes.sh — Verificacao de CI (Fase 5).
#
# Falha quando o diff contem mudancas relevantes sem um registro novo em
# `changes/` (convecao `changes/YYYY-MM-DD-<tipo>-<slug>.md`).
#
# Uso:
#   tools/check-changes.sh [base] [--bypass "motivo explicito"]
#
# `base` padrao: origin/main. Compara `base...HEAD`.
# Em CI, use `fetch-depth: 0` para que o historico de `base` exista.
# `--bypass` registra excecao explicita e justificada (ex.: tipografia pura).

set -u

BASE="origin/main"
BYPASS=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --bypass)
      shift
      BYPASS="${1:-}"
      if [ -z "$BYPASS" ]; then
        echo "erro: --bypass exige motivo entre aspas" >&2
        exit 2
      fi
      ;;
    --bypass=*)
      BYPASS="${1#--bypass=}"
      ;;
    -h|--help)
      echo "uso: tools/check-changes.sh [base] [--bypass \"motivo explicito\"]"
      exit 0
      ;;
    *)
      BASE="$1"
      ;;
  esac
  shift
done

if [ -n "$BYPASS" ]; then
  echo "check-changes: bypass justificado: $BYPASS"
  exit 0
fi

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "check-changes: fora de um repositorio Git; nada a verificar" >&2
  exit 0
fi

CHANGED="$(git diff --name-only "$BASE"...HEAD 2>/dev/null || git diff --name-only "$BASE" 2>/dev/null || true)"
if [ -z "$CHANGED" ]; then
  echo "check-changes: sem diferencas; ok"
  exit 0
fi

RELEVANT="$(printf '%s\n' "$CHANGED" | grep -E '^(crates/|tools/|examples/|ADRs/|ARCHITECTURE\.md|LANGUAGE_SPEC\.md|BYTECODE_SPEC\.md|VM_SPEC\.md|MEMORY_MODEL\.md|PHP_DIFFERENCES\.md|ROADMAP\.md|CHANGELOG\.md|AGENTS\.md|GOVERNANCE\.md|hello\.rhp|Cargo\.toml|Cargo\.lock)' || true)"
if [ -z "$RELEVANT" ]; then
  echo "check-changes: so arquivos nao relevantes; ok"
  exit 0
fi

# Inclui adicionados e renomeados (--diff-filter=AR): um registro movido
# continua valendo como registro.
ADDED="$(git diff --name-only --diff-filter=AR "$BASE"...HEAD 2>/dev/null | grep -E '^changes/[0-9]{4}-[0-9]{2}-[0-9]{2}-.+\.md$' || true)"
if [ -z "$ADDED" ]; then
  echo "check-changes: FALHOU — mudancas relevantes sem registro novo em changes/:" >&2
  printf '%s\n' "$RELEVANT" >&2
  echo "Adicione changes/YYYY-MM-DD-<tipo>-<slug>.md ou use --bypass \"motivo\"." >&2
  exit 1
fi

echo "check-changes: ok (registro: $ADDED)"
