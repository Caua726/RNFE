//! nestest: compara registradores e ciclos, instrução a instrução, com o log de referência.
//!
//! `VERIFIED_LINES` é o progresso executável: sobe conforme os opcodes não-oficiais são
//! implementados (meta: 8991 = o log inteiro). O teste falha tanto se uma linha já
//! verificada divergir quanto se o emulador passar a bater MAIS linhas sem a constante subir.

use rnfe_core::testing::nestest::{St, TOTAL_LINES, compare};

/// Linhas do log que DEVEM bater. Meta: 8991.
const VERIFIED_LINES: usize = 5004;

#[test]
fn nestest_matches_log() {
    let r = match compare() {
        Ok(r) => r,
        Err(e) => {
            if std::env::var_os("RNFE_REQUIRE_ROMS").is_some() {
                panic!("{e}");
            }
            eprintln!("SKIP nestest: {e}");
            return;
        }
    };

    if let Some((i, ours)) = r.first_bad.filter(|(i, _)| *i < VERIFIED_LINES) {
        let head = |l: &str| l.split("A:").next().unwrap().trim_end().to_string();
        let ctx = (i.saturating_sub(3)..i)
            .map(|j| format!("      {:5} {}", j + 1, head(&r.log[j])))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "nestest divergiu na linha {} (verificadas: {VERIFIED_LINES})\n{ctx}\n  log: {}\n  nós: {}\n  instrução: {}",
            i + 1,
            St::parse(&r.log[i]).fmt(),
            ours.fmt(),
            head(&r.log[i])
        );
    }
    assert!(
        r.matched == VERIFIED_LINES || r.matched == TOTAL_LINES,
        "nestest agora bate {} linhas (> VERIFIED_LINES = {VERIFIED_LINES}) — suba a constante em tests/nestest.rs",
        r.matched
    );
    if r.matched == TOTAL_LINES {
        assert_eq!(r.result_bytes, (0, 0), "nestest reportou erro em $02/$03");
    }
}
