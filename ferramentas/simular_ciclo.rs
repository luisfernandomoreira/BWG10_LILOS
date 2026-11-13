//! Roda a máquina de estados REAL (`src/maquina.rs`) no PC e confere se ela
//! percorre todas as etapas de cada programa e desliga no fim.
//!
//! Compilar e rodar, na raiz do projeto:
//!
//! ```text
//! rustc -O ferramentas/simular_ciclo.rs -o simular && ./simular
//! ```
//!
//! Não é um teste de uma cópia do código: `maquina.rs`, `programa.rs`,
//! `agitacao.rs` e `opcoes.rs` são incluídos do `src/` como módulos.
//! Só o que toca hardware (`placa`), a fila de eventos (`evento`, que
//! depende do lilos) e a gravação na flash (`estado_salvo`) são
//! substituídos por dublês aqui embaixo.
//!
//! Se um dia alguém acrescentar uma variante em `Evento` e tratá-la na
//! máquina, este arquivo para de compilar — que é o aviso desejado.
//!
//! O que é verificado, para os 5 programas × com e sem enxágue extra:
//!
//!   1. a máquina entra em execução depois do liga/desliga;
//!   2. passa por TODAS as etapas aplicáveis da tabela, em ordem,
//!      sem pular nenhuma e sem repetir;
//!   3. chega ao fim da tabela;
//!   4. termina com todos os triacs desligados;
//!   5. no fim de CENTRIFUGAR: volta para lavagem pesada, zera as
//!      opções e fica DESLIGADA — igual ao firmware original;
//!   6. nos outros programas: termina em "concluída", ainda ligada,
//!      esperando o seletor.

#![allow(dead_code)]

// ═════════════════════════════════════════════════════════════════════════════
// DUBLÊS — o que a máquina de estados espera do resto do firmware
// ═════════════════════════════════════════════════════════════════════════════

mod evento {
    //! Só os tipos. A fila de verdade depende do lilos e não é usada aqui:
    //! o simulador chama `maquina.trata(...)` diretamente.

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Botao {
        OnOff = 0,
        Seletor = 1,
        Nivel = 2,
        EnxagueExtra = 3,
        Turbo = 4,
        Amaciante = 5,
    }

    impl Botao {
        pub const TODOS: [Botao; 6] = [
            Botao::OnOff,
            Botao::Seletor,
            Botao::Nivel,
            Botao::EnxagueExtra,
            Botao::Turbo,
            Botao::Amaciante,
        ];
        pub const fn indice(self) -> usize {
            self as usize
        }
        pub const fn bit(self) -> u8 {
            1 << (self as u8)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Evento {
        BotaoPressionado(Botao),
        BotaoSolto(Botao),
        ComboAutoTeste,
        Tampa { aberta: bool },
        NivelMedio { atingido: bool },
        NivelAlto { atingido: bool },
        Tique,
        Segundo,
    }
}

mod estado_salvo {
    use crate::opcoes::Opcoes;
    use crate::programa::Programa;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct EstadoSalvo {
        pub programa: Programa,
        pub etapa: u8,
        pub opcoes: Opcoes,
        pub ligado: bool,
    }

    /// No firmware isto enfileira a gravação na flash. Aqui não faz nada.
    pub fn pede_gravacao(_estado: EstadoSalvo) {}
}

mod placa {
    //! Dublê da placa: em vez de mexer em pino, registra o estado.
    //!
    //! Mantém a lógica invertida do hardware real: `baixa()` liga.

    use crate::evento::Botao;
    use crate::opcoes::Opcoes;
    use crate::programa::Programa;
    use std::cell::Cell;

    pub struct Saida {
        pub ligada: Cell<bool>,
    }

    impl Saida {
        pub fn nova() -> Self {
            Self {
                ligada: Cell::new(false),
            }
        }
        pub fn alta(&self) {
            self.ligada.set(false); // nível alto = desligado
        }
        pub fn baixa(&self) {
            self.ligada.set(true); // nível baixo = ligado
        }
        pub fn escreve(&self, nivel_alto: bool) {
            self.ligada.set(!nivel_alto);
        }
        pub fn esta_ligada(&self) -> bool {
            self.ligada.get()
        }
    }

    pub struct Painel {
        pub pesada: Saida,
        pub normal: Saida,
        pub rapido: Saida,
        pub enxaguar: Saida,
        pub centrifugar: Saida,
        pub enxague_extra: Saida,
        pub turbo: Saida,
        pub amaciante: Saida,
        pub nivel_medio: Saida,
        pub nivel_alto: Saida,
    }

    impl Painel {
        pub fn nova() -> Self {
            Self {
                pesada: Saida::nova(),
                normal: Saida::nova(),
                rapido: Saida::nova(),
                enxaguar: Saida::nova(),
                centrifugar: Saida::nova(),
                enxague_extra: Saida::nova(),
                turbo: Saida::nova(),
                amaciante: Saida::nova(),
                nivel_medio: Saida::nova(),
                nivel_alto: Saida::nova(),
            }
        }

        fn programas(&self) -> [&Saida; 5] {
            [
                &self.pesada,
                &self.normal,
                &self.rapido,
                &self.enxaguar,
                &self.centrifugar,
            ]
        }

        pub fn todos(&self) -> [&Saida; 10] {
            [
                &self.pesada,
                &self.normal,
                &self.rapido,
                &self.enxaguar,
                &self.centrifugar,
                &self.enxague_extra,
                &self.turbo,
                &self.amaciante,
                &self.nivel_medio,
                &self.nivel_alto,
            ]
        }

        pub fn apaga_programas(&self) {
            for l in self.programas() {
                l.alta();
            }
        }
        pub fn acende_programas(&self) {
            for l in self.programas() {
                l.baixa();
            }
        }
        pub fn acende_tudo(&self) {
            for l in self.todos() {
                l.baixa();
            }
        }
        pub fn apaga_tudo(&self) {
            for l in self.todos() {
                l.alta();
            }
        }
        pub fn mostra_programa(&self, p: Programa) {
            self.apaga_programas();
            self.programas()[p.indice() as usize].baixa();
        }
        pub fn mostra_opcoes(&self, o: &Opcoes, ligada: bool) {
            let v = |x: bool| ligada && x;
            self.enxague_extra.escreve(!v(o.enxague_extra));
            self.turbo.escreve(!v(o.turbo));
            self.amaciante.escreve(!v(o.amaciante));
            self.nivel_medio.escreve(!v(ligada));
            self.nivel_alto.escreve(!v(o.nivel_alto));
        }
    }

    pub struct Atuadores {
        pub motor_c: Saida,
        pub motor_a: Saida,
        pub eletrobomba: Saida,
        pub valvula_sabao: Saida,
        pub valvula_amaciante: Saida,
    }

    impl Atuadores {
        pub fn novos() -> Self {
            Self {
                motor_c: Saida::nova(),
                motor_a: Saida::nova(),
                eletrobomba: Saida::nova(),
                valvula_sabao: Saida::nova(),
                valvula_amaciante: Saida::nova(),
            }
        }
        pub fn desliga_todos(&self) {
            self.motor_c.alta();
            self.motor_a.alta();
            self.eletrobomba.alta();
            self.valvula_sabao.alta();
            self.valvula_amaciante.alta();
        }
        pub fn motor_a(&self, ligar: bool) {
            self.motor_a.escreve(!ligar);
        }
        pub fn motor_c(&self, ligar: bool) {
            self.motor_c.escreve(!ligar);
        }
        pub fn eletrobomba(&self, ligar: bool) {
            self.eletrobomba.escreve(!ligar);
        }
        pub fn valvula_sabao(&self, abrir: bool) {
            self.valvula_sabao.escreve(!abrir);
        }
        pub fn valvula_amaciante(&self, abrir: bool) {
            self.valvula_amaciante.escreve(!abrir);
        }
        pub fn algum_ligado(&self) -> bool {
            self.motor_c.esta_ligada()
                || self.motor_a.esta_ligada()
                || self.eletrobomba.esta_ligada()
                || self.valvula_sabao.esta_ligada()
                || self.valvula_amaciante.esta_ligada()
        }
    }

    pub struct Entradas {
        pub nivel_medio: Cell<bool>,
        pub nivel_alto: Cell<bool>,
        pub tampa: Cell<bool>,
    }

    impl Entradas {
        pub fn novas() -> Self {
            Self {
                nivel_medio: Cell::new(false),
                nivel_alto: Cell::new(false),
                tampa: Cell::new(false),
            }
        }
        pub fn botao_pressionado(&self, _b: Botao) -> bool {
            false
        }
        pub fn nivel_medio_atingido(&self) -> bool {
            self.nivel_medio.get()
        }
        pub fn nivel_alto_atingido(&self) -> bool {
            self.nivel_alto.get()
        }
        pub fn tampa_aberta(&self) -> bool {
            self.tampa.get()
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// MÓDULOS REAIS DO FIRMWARE
// ═════════════════════════════════════════════════════════════════════════════

#[path = "../src/agitacao.rs"]
mod agitacao;
#[path = "../src/maquina.rs"]
mod maquina;
#[path = "../src/opcoes.rs"]
mod opcoes;
#[path = "../src/programa.rs"]
mod programa;

use estado_salvo::EstadoSalvo;
use evento::{Botao, Evento};
use opcoes::Opcoes;
use placa::{Atuadores, Entradas, Painel};
use programa::Programa;

// ═════════════════════════════════════════════════════════════════════════════
// MODELO DO TANQUE
// ═════════════════════════════════════════════════════════════════════════════

/// Enchimento/drenagem em unidades arbitrárias de 0 a 100 por segundo.
/// Os tempos são curtos de propósito: o que se verifica aqui é a SEQUÊNCIA
/// de etapas, não a duração delas (isso é `conferir_programas.rs`).
struct Tanque {
    nivel: i32,
}

const NIVEL_MEDIO: i32 = 40;
const NIVEL_ALTO: i32 = 80;

impl Tanque {
    fn novo(nivel: i32) -> Self {
        Self { nivel }
    }

    /// Avança um segundo e devolve `(medio, alto)`.
    fn segundo(&mut self, at: &Atuadores) -> (bool, bool) {
        if at.valvula_sabao.esta_ligada() || at.valvula_amaciante.esta_ligada() {
            self.nivel += 4;
        }
        if at.eletrobomba.esta_ligada() {
            self.nivel -= 5;
        }
        self.nivel = self.nivel.clamp(0, 100);
        (self.nivel >= NIVEL_MEDIO, self.nivel >= NIVEL_ALTO)
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// SIMULAÇÃO
// ═════════════════════════════════════════════════════════════════════════════

const TIQUES_POR_SEGUNDO: u32 = 20;
/// Teto de segurança: 6 horas simuladas.
const LIMITE_SEGUNDOS: u32 = 6 * 60 * 60;

struct Resultado {
    etapas_vistas: Vec<(Programa, usize)>,
    segundos: u32,
    terminou: bool,
    desligada: bool,
    triacs_ligados: bool,
    programa_final: Programa,
    opcoes_finais: Opcoes,
}

fn simula(programa: Programa, enxague_extra: bool, nivel_inicial: i32) -> Resultado {
    let painel = Painel::nova();
    let at = Atuadores::novos();
    let entradas = Entradas::novas();
    // A máquina lê os sensores uma vez no construtor, para nascer com o
    // espelho certo — então o nível inicial tem de estar posto antes.
    entradas.nivel_medio.set(nivel_inicial >= NIVEL_MEDIO);
    entradas.nivel_alto.set(nivel_inicial >= NIVEL_ALTO);
    let mut m = maquina::Maquina::nova(&painel, &at, &entradas);

    // Coloca o programa a testar (e as opções) como se viesse da flash.
    m.restaura(EstadoSalvo {
        programa,
        etapa: 0,
        opcoes: Opcoes {
            nivel_alto: false,
            enxague_extra,
            turbo: false,
            amaciante: false,
        },
        ligado: false,
    });

    let mut tanque = Tanque::novo(nivel_inicial);
    let mut medio_antes = nivel_inicial >= NIVEL_MEDIO;
    let mut alto_antes = nivel_inicial >= NIVEL_ALTO;

    // ── Deixa a animação de inicialização terminar ───────────────────────────
    for _ in 0..(10 * TIQUES_POR_SEGUNDO) {
        m.trata(Evento::Tique);
        if m.fotografia().desligada {
            break;
        }
    }

    // ── Liga a máquina ───────────────────────────────────────────────────────
    m.trata(Evento::BotaoPressionado(Botao::OnOff));
    m.trata(Evento::BotaoSolto(Botao::OnOff));

    let mut etapas_vistas: Vec<(Programa, usize)> = Vec::new();
    let mut etapa_anterior: Option<(Programa, usize)> = None;
    let mut entrou_em_execucao = false;
    let mut segundos = 0u32;
    let mut terminou = false;

    // Amostrar a cada TIQUE (50 ms), não a cada segundo: etapas que
    // terminam em menos de um segundo — uma drenagem com o tanque já
    // vazio, por exemplo — existem de verdade e precisam aparecer.
    let registra = |m: &maquina::Maquina,
                    vistas: &mut Vec<(Programa, usize)>,
                    anterior: &mut Option<(Programa, usize)>,
                    entrou: &mut bool| {
        let f = m.fotografia();
        if f.executando {
            *entrou = true;
            let agora = (f.programa, f.etapa);
            if *anterior != Some(agora) {
                vistas.push(agora);
                *anterior = Some(agora);
            }
        }
    };

    while segundos < LIMITE_SEGUNDOS {
        for _ in 0..TIQUES_POR_SEGUNDO {
            m.trata(Evento::Tique);
            registra(
                &m,
                &mut etapas_vistas,
                &mut etapa_anterior,
                &mut entrou_em_execucao,
            );
        }

        // Sensores de nível: só publicam quando mudam, como na placa.
        let (medio, alto) = tanque.segundo(&at);
        if medio != medio_antes {
            m.trata(Evento::NivelMedio { atingido: medio });
            medio_antes = medio;
        }
        if alto != alto_antes {
            m.trata(Evento::NivelAlto { atingido: alto });
            alto_antes = alto;
        }

        m.trata(Evento::Segundo);
        segundos += 1;
        registra(
            &m,
            &mut etapas_vistas,
            &mut etapa_anterior,
            &mut entrou_em_execucao,
        );

        if entrou_em_execucao && !m.fotografia().executando {
            terminou = true;
            break;
        }
    }

    let f = m.fotografia();
    Resultado {
        etapas_vistas,
        segundos,
        terminou,
        desligada: f.desligada,
        triacs_ligados: at.algum_ligado(),
        programa_final: f.programa,
        opcoes_finais: f.opcoes,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// VERIFICAÇÃO
// ═════════════════════════════════════════════════════════════════════════════

/// Como o tanque está quando o usuário aperta o liga/desliga.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cenario {
    /// Tanque vazio — a situação normal.
    Vazio,
    /// Tanque com água parada (ciclo interrompido, energia caiu cheia).
    Cheio,
}

impl Cenario {
    fn nivel(self) -> i32 {
        match self {
            Cenario::Vazio => 0,
            Cenario::Cheio => 100,
        }
    }
    fn nome(self) -> &'static str {
        match self {
            Cenario::Vazio => "tanque vazio",
            Cenario::Cheio => "tanque cheio",
        }
    }
}

/// A cadeia de programas que roda a partir de `inicio`, até a
/// centrifugação encerrar o ciclo.
fn cadeia(inicio: Programa) -> Vec<Programa> {
    let mut v = vec![inicio];
    let mut p = inicio;
    // Limite pelo número de programas: nunca gira sem fim.
    for _ in 0..Programa::QUANTIDADE {
        if p.fim_de_ciclo() == programa::FimDeCiclo::Desliga {
            break;
        }
        p = p.proximo();
        v.push(p);
    }
    v
}

/// Etapas que a máquina deve percorrer, na ordem, em toda a cadeia.
fn etapas_esperadas(inicio: Programa, extra: bool, cenario: Cenario) -> Vec<(Programa, usize)> {
    let mut esperadas = Vec::new();

    for (pos, p) in cadeia(inicio).into_iter().enumerate() {
        // O ENXAGUAR começa com um bloco de drenagem que só roda se
        // houver água — igual ao PASSO 0 do `enxaguar` original ("drena
        // se houver água"). Isso só acontece quando ele é o primeiro da
        // cadeia E o tanque já estava cheio; vindo do programa anterior,
        // a bomba acabou de esvaziar o tanque.
        let primeira_etapa = if p == Programa::Enxaguar
            && !(pos == 0 && cenario == Cenario::Cheio)
        {
            5
        } else {
            0
        };

        for (i, et) in p.etapas().iter().enumerate() {
            if i >= primeira_etapa && (!et.so_enxague_extra || extra) {
                esperadas.push((p, i));
            }
        }
    }

    esperadas
}

fn verifica(inicio: Programa, extra: bool, cenario: Cenario) -> Vec<String> {
    let mut falhas = Vec::new();
    let r = simula(inicio, extra, cenario.nivel());
    let esperadas = etapas_esperadas(inicio, extra, cenario);

    let nomes_cadeia: Vec<String> = cadeia(inicio).iter().map(|p| format!("{p:?}")).collect();
    println!(
        "
=== começando em {:?} — enxágue extra {} — {} — {} min simulados",
        inicio,
        if extra { "LIGADO" } else { "desligado" },
        cenario.nome(),
        r.segundos / 60
    );
    println!("  cadeia esperada: {}", nomes_cadeia.join(" → "));

    // Imprime a sequência agrupada por programa, para caber na tela.
    let mut linha = String::new();
    let mut prog_atual: Option<Programa> = None;
    for &(p, i) in &r.etapas_vistas {
        if prog_atual != Some(p) {
            if prog_atual.is_some() {
                linha.push_str("
              ");
            }
            linha.push_str(&format!("{p:?}: "));
            prog_atual = Some(p);
        }
        let nome = match p.etapas().get(i) {
            Some(et) => format!("{:?}", et.acao),
            None => "??".to_string(),
        };
        linha.push_str(&format!("{i}:{nome} "));
    }
    println!("  percorreu:    {linha}");

    if !r.terminou {
        falhas.push(format!(
            "não terminou em {} min simulados (travou numa etapa?)",
            r.segundos / 60
        ));
    }

    // 1. Todas as etapas da cadeia, na ordem, sem pular e sem repetir.
    if r.etapas_vistas != esperadas {
        let faltando: Vec<String> = esperadas
            .iter()
            .filter(|e| !r.etapas_vistas.contains(e))
            .map(|(p, i)| format!("{p:?}[{i}]"))
            .collect();
        if faltando.is_empty() {
            falhas.push(format!(
                "ordem ou repetição errada: {} etapas percorridas, {} esperadas",
                r.etapas_vistas.len(),
                esperadas.len()
            ));
        } else {
            falhas.push(format!("etapas que nunca rodaram: {}", faltando.join(", ")));
        }
    }

    // 2. A cadeia tem de passar pela centrifugação.
    let centrifugou = r
        .etapas_vistas
        .iter()
        .any(|&(p, _)| p == Programa::Centrifugar);
    if !centrifugou {
        falhas.push("a cadeia nunca chegou na centrifugação".to_string());
    }

    // 3. Nada de triac ligado depois do fim.
    if r.triacs_ligados {
        falhas.push("terminou com algum triac ainda ligado".to_string());
    }

    // 4. O fim da cadeia desliga e zera tudo, como no original.
    if !r.desligada {
        falhas.push("fim da centrifugação não deixou a máquina DESLIGADA".to_string());
    }
    if r.programa_final != Programa::Pesada {
        falhas.push(format!(
            "fim da centrifugação deixou o programa em {:?}, deveria voltar para Pesada",
            r.programa_final
        ));
    }
    if r.opcoes_finais != Opcoes::padrao() {
        falhas.push("fim da centrifugação não zerou as opções".to_string());
    }

    falhas
}

const TODOS: [Programa; 5] = [
    Programa::Pesada,
    Programa::Normal,
    Programa::Rapido,
    Programa::Enxaguar,
    Programa::Centrifugar,
];

fn main() {
    let mut problemas = 0;

    let mut rodados = 0;
    for p in TODOS {
        for extra in [false, true] {
            for cenario in [Cenario::Vazio, Cenario::Cheio] {
            rodados += 1;
            let falhas = verifica(p, extra, cenario);
            if falhas.is_empty() {
                println!("  ✔ sem problemas");
            } else {
                problemas += falhas.len();
                for f in falhas {
                    println!("  ✘ {f}");
                }
            }
            }
        }
    }

    println!();
    if problemas == 0 {
        println!("Tudo certo: {rodados} ciclos simulados de ponta a ponta.");
    } else {
        println!("{problemas} problema(s) encontrado(s).");
        std::process::exit(1);
    }
}
