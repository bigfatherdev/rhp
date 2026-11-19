# Concorrencia no RHP — modelo, limites e padroes

Este guia consolida o modelo de tarefas do RHP (RFCs 0019, 0023, 0030,
item 1 e metodos-async): o que atravessa a fronteira da thread, o que nao
atravessa (por decisao arquitetural permanente) e os padroes recomendados.

## O modelo em uma frase

> **Tarefas compartilham CODIGO (definicoes de topo), nao ESTADO
> (globais mutaveis).** Cada tarefa roda em thread propria, com programa
> recompilado: executa as definicoes (`fn`, `class`, `interface`, `trait`,
> `namespace`, `import native`) e depois a funcao-alvo com args + captura.

## O que atravessa a fronteira

| Dado | Atravessa? | Como |
|---|---|---|
| Argumentos posicionais planos | sim | JSON |
| Captura (closure) plana | sim | JSON (somentee o que e plano; resto fica indisponivel) |
| Instancia (arg, captura, retorno) | sim | envelope com classe (`__rhp_instancia__`); reconstruida no outro lado |
| Funcao de topo chamada pela tarefa | sim | codigo compartilhado (nome resolvido no worker) |
| Classe usada pela tarefa | sim | definida no worker (mesmo programa) |
| Metodos `async` (`this`) | sim | snapshot: copia com classe; **sem write-back** |
| Globais mutaveis (`let` no topo) | **nao** | decisao permanente (ver abaixo) |
| Funcao/classe/gerador como valor | nao | erro claro (`nao transporta ...`) |
| Mapa com chave `__rhp_instancia__` | nao | erro claro (reservada ao envelope) |

Retornos voltam pelo mesmo caminho (JSON + decodificacao com a classe
local); instancia devolvida vira instancia de verdade, nao mapa puro.

## Por que globais mutaveis nao atravessam (decisao permanente)

Valores RHP usam `Rc` (nao `Send`/`Sync`); compartilha-los entre threads
exigiria redesenhar `Value` para `Arc`+`Mutex` em todos os caminhos da VM —
custo e risco desproporcionais ao beneficio, com semantica de corrida dificil
de documentar e testar. Em vez disso, o RHP oferece composicao explicita
(fan-out com args, fan-in com `await`/`await_all`/`await_race`) e cancelamento
cooperativo. Reavaliar exigiria RFC propria com prova de seguranca de threads.

## Padroes

### Fan-out / fan-in

```rhp
async fn buscarA() { sleep(0.5); return "A"; }
async fn buscarB() { return "B"; }
let a = buscarA();   // nao bloqueia
let b = buscarB();   // nao bloqueia
echo await a;        // junta quando pronto
echo await b;
```

### Timeout com reaproveitamento

```rhp
let h = buscarA();
try {
  echo await_timeout(h, 1);
} catch e {
  echo "tempo esgotado";
}
echo await h; // a tarefa seguiu em voo; junta aqui
```

### Cancelamento cooperativo

```rhp
async fn trabalho() {
  let i = 0;
  while i < 100000000 {
    if async_cancelled() { return "cancelado"; }
    i = i + 1;
  }
  return "ok";
}
let h = trabalho();
async_cancel(h);   // so sinaliza; a tarefa precisa sondar
echo await h;      // "cancelado"
```

### Agregacao e corrida

```rhp
let todos = await_all([buscarA(), buscarB()]); // ["A", "B"], na ordem
let primeiro = await_race([buscarA(), buscarB()]); // o mais rapido vence
```

### Metodos async (snapshot)

```rhp
class Contador {
  fn novo() { this["n"] = 0; }
  async fn soma(x) { return this["n"] + x; }
}
let c = new Contador();
echo await c.soma(5); // 5; `c` continua com n = 0 (sem write-back)
```

Mutacoes feitas na tarefa NAO voltam ao chamador. Devolva valores com
`return` em vez de mutar `this`. Detach (`let f = obj.m; await f()`) e
`super.m()` async seguem a mesma regra. Nomeados/spread em chamada async
= erro (use posicionais).

### `await` de nao-tarefa e identidade

`await 42` devolve `42`. Consequencia: apos consumir um handle (`await h`),
`h` e so um `Int` — um segundo `await h` devolve o numero, sem erro.
`async_join` continua one-shot com erro (`handle invalido ou ja consumido`).

## Limites e operacao

- Uma thread por tarefa (sem pool): faca `join`/`await` em todas; nao crie
  tarefas em loop sem limite.
- Sem preempcao: `async_cancel` so sinaliza; tarefa presa sem sondar nao
  para (projete sondagens em lacos longos).
- Timeout nao mata a tarefa (segue destacada); junte depois ou deixe
  terminar. Tarefas nunca juntadas vazam o handle ate o fim do programa.
- Erros da tarefa viram erros no `join`/`await` (`tarefa ... falhou: ...`);
  use `try`/`catch` normalmente.
