# [Docs] — Fase 0: auditoria de paridade com PHP 8.5

- ID: 2026-11-14-docs-php-parity-audit
- Data: 2026-11-14
- Versão: unreleased
- Tipo: Docs
- Compatibilidade: Compatível (só adiciona `reports/`; zero mudança de código)

## Resumo

Auditoria integral para paridade funcional com PHP 8.5.12 (referência
oficial verificada hoje: GA 20/11/2025, suporte até 31/12/2027):
`reports/PHP_PARITY_AUDIT.md` com estado real, matriz RHP×PHP, lacunas,
dependências, riscos, o que é imediato / exige preparo / não deve ser
feito, roadmap e complexidade por módulo.

## Motivação

Missão Next Generation, Fase 0: auditar antes de modificar; apresentar
resultado e aguardar aprovação para mudanças arquiteturais.

## O que mudou

- `reports/PHP_PARITY_AUDIT.md` (novo).

## Detalhes técnicos

Método: leitura integral do workspace + specs + ADRs + changes/ +
relatórios; sondas em `/tmp` (repo intacto). Referência cruzada com
php.net/downloads, php.watch, php.net/archive/2026 (8.5.12; 8.6 em RC,
fora do escopo).

## Compatibilidade e migração

Compatível. Migração: não necessária.

## Segurança e performance

Nenhuma mudança de comportamento (só leitura + relatório).

## Testes e validação

- `cargo fmt --check`: ok. `cargo clippy ... -D warnings`: ok.
- `cargo test --workspace`: 133 testes, 0 falhas (estado inalterado).
- `git status`: só o relatório novo + este registro.

## Limitações e riscos conhecidos

Auditoria não executa rede externa nem Windows. Implementações futuras
exigem aprovação explícita por item (governança + missão §22).

## Referências

Missão Next Generation, Fase 0. Sem RFC/ADR (auditoria não muda semântica).
